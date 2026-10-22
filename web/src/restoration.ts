import type { Controller } from './controller.ts';
import { continuation } from './client.ts';
import { busReady, itemGeneration, rows, type Snapshot, type PhysicalRow } from './model.ts';

export class Restoration {
  readonly ui:Controller;readonly initial:Snapshot;readonly epoch:number;readonly original:PhysicalRow;
  readonly space:'outputs'|'inputs';readonly candidates=new Set<string>();
  revision:number;address:number;generation:string;cancelled=false;
  constructor(ui:Controller,row:PhysicalRow){
    if(row.kind!=='output'&&row.kind!=='input')throw new Error('Select a saved fixture or input module.');
    this.ui=ui;this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;
    this.original=structuredClone(row);this.space=row.kind==='output'?'outputs':'inputs';this.revision=this.initial.inventory.revision;
    this.address=row.item.short_address;this.generation=row.generation;
    for(const candidate of ui.currentRows)if(candidate.kind===row.kind&&candidate.bus.bus===row.bus.bus&&candidate.key!==row.key)this.candidates.add(candidate.key);
  }
  private live(){
    if(this.cancelled||this.ui.cancelled||this.ui.signingOut||!this.ui.client.views.ready||this.ui.client.views.epoch!==this.epoch)
      throw new Error('Address recovery stopped. Completed changes remain saved; inspect before retrying.');
  }
  assert(){
    this.live();const snapshot=this.ui.snapshot,bus=snapshot.inventory.buses.find(b=>b.bus===this.original.bus.bus);
    const current=rows(snapshot).find(r=>r.key===this.original.key);
    if(!continuation(this.initial,snapshot,this.revision,this.original.bus.bus)||!bus||!busReady(snapshot,bus)||!current||current.kind==='scene'||current.item.target!==this.original.item.target||current.generation!==this.generation||current.item.short_address!==this.address)
      throw new Error('Configuration or saved fixture changed. Review before continuing address recovery.');
  }
  async accept(revision:unknown,mapping?:{address:number;generation:string}){
    this.live();if(!Number.isSafeInteger(revision)||Number(revision)<this.revision)throw new Error('Address recovery result changed. Inspect before retrying.');
    await this.ui.changed(Number(revision));this.revision=Number(revision);if(mapping){this.address=mapping.address;this.generation=mapping.generation;}this.assert();
  }
  private async controller(command:string,params:Record<string,unknown>){
    this.assert();const result=await this.ui.client.job(itemGeneration(this.initial.inventory.inventory_epoch,this.revision),null,command,{...params,bus:this.original.bus.bus,expected_revision:this.revision});
    const identity=result.probe??result;
    if(identity.bus!==this.original.bus.bus||identity.space!==this.space)throw new Error('Candidate discovery result changed. Inspect saved devices.');
    await this.accept(result.revision);return result;
  }
  async find(){
    const result=await this.controller(this.space==='outputs'?'assign_next_output':'assign_next_input',{});
    if(result.outcome==='no_free_address')throw new Error('No free temporary address. Use the sole-unaddressed option after deliberately isolating the intended fixture.');
    if(result.outcome===`no_unaddressed_${this.space==='outputs'?'output':'input'}`){this.ui.notice='No unaddressed fixture found. Completed candidates remain on this list.';return;}
    if(result.outcome!=='assigned'||!Number.isInteger(result.address)||result.address<0||result.address>63)throw new Error('Temporary address is unconfirmed. Inspect the saved reservation before continuing.');
    const address=result.address;
    await this.controller('discover_address',{space:this.space,address});
    const row=this.ui.currentRows.find(r=>r.kind===this.original.kind&&r.bus.bus===this.original.bus.bus&&r.item.short_address===address);
    if(!row||row.key===this.original.key)throw new Error('Temporary candidate changed. Inspect the saved reservation.');
    this.candidates.add(row.key);
    for(const current of this.ui.currentRows.filter(r=>r.bus.bus===row.bus.bus&&(row.kind==='output'?r.key===row.key:r.kind==='instance'&&r.parent?.short_address===address))){this.assert();await this.ui.read(current);this.assert();}
    this.ui.notice='Candidate added. Blink it or operate its inputs, then select Use this fixture on its row.';
  }
  review(candidate?:PhysicalRow){
    this.assert();
    if(candidate){
      const current=this.ui.currentRows.find(r=>r.key===candidate.key);
      if(!this.candidates.has(candidate.key)||!current||current.kind!==this.original.kind||current.bus.bus!==this.original.bus.bus||current.item.target!==candidate.item.target||current.generation!==candidate.generation)
        throw new Error('Identified candidate changed. Review it again.');
    }
    return new RestorationReview(this,candidate);
  }
  async refresh(){
    try{
      await this.controller('discover_address',{space:this.space,address:this.address});
      for(const row of this.ui.currentRows.filter(r=>r.bus.bus===this.original.bus.bus&&(this.space==='outputs'?r.key===this.original.key:r.kind==='instance'&&r.parent?.short_address===this.address))){this.assert();await this.ui.read(row);this.assert();}
      this.ui.notice=`Address restored at ${this.address}. Saved name and bindings preserved; current values refreshed.`;
    }catch{this.ui.notice=`Address restored at ${this.address}. Current-value refresh did not finish; use Refresh addressed or Read current values.`;}
  }
}

export class RestorationReview {
  readonly scope:Restoration;readonly revision:number;readonly candidate?:PhysicalRow;
  constructor(scope:Restoration,candidate?:PhysicalRow){this.scope=scope;this.revision=scope.revision;this.candidate=candidate?structuredClone(candidate):undefined;}
  async run(){
    const scope=this.scope,ui=scope.ui,old=scope.original,selected=this.candidate;
    const check=()=>{scope.assert();if(scope.revision!==this.revision)throw new Error('Recovery review changed. Review both fixtures again.');if(selected){const now=ui.currentRows.find(r=>r.key===selected.key);if(!now||now.item.target!==selected.item.target||now.generation!==selected.generation)throw new Error('Identified candidate changed. Review it again.');}};
    check();
    const blink=ui.snapshot.identification;
    if(blink&&['starting','blinking','restoring'].includes(blink.phase)){
      if(!selected||blink.target!==selected.item.target||blink.generation!==selected.generation)throw new Error('Stop the other blinking fixture before restoring an address.');
      const stopped=await ui.client.job(selected.generation,selected.item.target,'stop_blink',{});
      if(stopped.phase!=='restored')throw new Error('Blink restoration is unconfirmed. Address recovery stopped.');
      check();
    }
    const command=selected?'restore_identified_address':'restore_unaddressed_address';
    const result=await ui.client.job(old.generation,old.item.target,command,{expected_revision:this.revision,...selected?{candidate:selected.item.target,candidate_generation:selected.generation}:{}});
    const address=selected?.item.short_address??old.item.short_address;
    const generation=selected?itemGeneration(scope.initial.inventory.inventory_epoch,result.revision):old.generation;
    if(result.operation!==command||result.target!==old.item.target||result.target_generation!==old.generation||result.current_generation!==generation||result.bus!==old.bus.bus||result.previous_address!==old.item.short_address||result.address!==address||selected&&(result.candidate!==selected.item.target||result.candidate_generation!==selected.generation||result.source_address!==selected.item.short_address))
      throw new Error('Address restoration result changed. Inspect hardware before retrying.');
    // Unique repair legitimately returns the unchanged root. Candidate retirement
    // may skip reserved revisions; the returned own root must still match SSE.
    await scope.accept(result.revision,{address,generation});
    if(result.outcome!=='restored'){
      if(result.outcome==='no_unaddressed_fixture')throw new Error('No unaddressed fixture found. No address was programmed.');
      if(result.outcome==='multiple_unaddressed_fixtures')throw new Error('Multiple unaddressed fixtures found. Identify candidates or isolate the intended fixture; no address was programmed.');
      throw new Error(result.outcome==='configuration_updated_but_unconfirmed'?`Saved address updated to ${address}; the previous reservation is free, but current fixture evidence is unconfirmed. Refresh addressed devices before retrying.`:'Address restoration is unconfirmed. Inspect hardware before retrying.');
    }
    if(selected&&ui.currentRows.some(r=>r.key===selected.key))throw new Error('Address restored but candidate retirement is not visible. Inspect saved configuration.');
    await scope.refresh();
  }
}

/** Frozen individual clearing; saved identities and their reservation continue. */
export class ClearAddress {
  readonly scope:Restoration;
  constructor(ui:Controller,row:PhysicalRow){this.scope=new Restoration(ui,row);}
  async run(){
    const scope=this.scope,ui=scope.ui,old=scope.original;
    scope.assert();
    const blink=ui.snapshot.identification;
    if(blink&&['starting','blinking','restoring'].includes(blink.phase)){
      if(blink.target!==old.item.target||blink.generation!==old.generation)throw new Error('Stop the other blinking fixture before clearing an address.');
      const stopped=await ui.client.job(old.generation,old.item.target,'stop_blink',{});
      if(stopped.phase!=='restored')throw new Error('Blink restoration is unconfirmed. Address clearing stopped.');
      scope.assert();
    }
    const result=await ui.client.job(old.generation,old.item.target,'clear_short_address',{expected_revision:scope.revision});
    if(result.operation!=='clear_short_address'||result.target!==old.item.target||result.target_generation!==old.generation||result.current_generation!==old.generation||result.bus!==old.bus.bus||result.previous_address!==old.item.short_address||result.address!==old.item.short_address||result.revision!==scope.revision||result.configuration_changed!==false)
      throw new Error('Address clearing result changed. Inspect hardware before retrying.');
    await scope.accept(result.revision);
    if(result.outcome!=='cleared')throw new Error('Address clearing is unconfirmed. Inspect hardware before retrying.');
    ui.notice='Short address cleared. Saved name and bindings retained. Use Add unaddressed, then Restore lost address to identify it again.';
  }
}
