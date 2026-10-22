import type { Controller } from './controller.ts';
import { continuation } from './client.ts';
import { rows, itemGeneration, busReady, type Snapshot } from './model.ts';

export type Space='outputs'|'inputs';
export type Selection=Space|'both';
export class Reassignment {
  readonly ui:Controller;readonly initial:Snapshot;readonly epoch:number;readonly bus:number;readonly spaces:Space[];
  revision:number;cancelled=false;
  constructor(ui:Controller,bus:number,selection:Selection){
    this.ui=ui;this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;this.bus=bus;
    this.revision=this.initial.inventory.revision;this.spaces=selection==='both'?['outputs','inputs']:[selection];
  }
  assert(){
    const ui=this.ui,s=ui.snapshot,bus=s.inventory.buses.find(b=>b.bus===this.bus);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Reassignment stopped. Completed changes remain saved.');
    if(!continuation(this.initial,s,this.revision,this.bus)||s.phase!=='ready'||!bus||!busReady(s,bus))throw new Error('Configuration or bus connection changed. Completed changes remain saved; review before continuing.');
  }
  private async perform(command:string,space:Space,address?:number){
    this.assert();
    const result=await this.ui.client.job(itemGeneration(this.initial.inventory.inventory_epoch,this.revision),null,command,{bus:this.bus,...command.startsWith('assign_next_')?{}:{space},...address===undefined?{}:{address},expected_revision:this.revision});
    const identity=result.probe??result;
    if(!Number.isSafeInteger(result.revision)||result.revision<this.revision||identity.bus!==this.bus||identity.space!==space)throw new Error('Reassignment result changed. Inspect configuration before retrying.');
    if(this.cancelled||this.ui.cancelled||this.ui.signingOut||this.epoch!==this.ui.client.views.epoch||!this.ui.client.views.ready)throw new Error('Reassignment stopped. Inspect saved changes before retrying.');
    await this.ui.changed(result.revision);this.revision=result.revision;this.assert();
    if(result.probe?.types_complete&&result.target){const row=this.ui.currentRows.find(r=>r.item.target===result.target);if(row)this.ui.hints.set(row.key,result.probe.device_types);}
    return result;
  }
  async run(progress:(text:string)=>void){
    for(const space of this.spaces){
      progress(`Retiring saved ${space} and clearing their addresses`);
      const reset=await this.perform('reset_address_space',space);
      if(reset.outcome!=='cleared')throw new Error('Saved identities were retired, but address clearing is unconfirmed. Reassignment stopped; review before retrying.');
      let full=true;
      for(let count=0;count<64;count++){
        progress(`Assigning ${space} · ${count} found`);
        const result=await this.perform(space==='outputs'?'assign_next_output':'assign_next_input',space);
        if(result.outcome===`no_unaddressed_${space==='outputs'?'output':'input'}`){full=false;break;}
        if(result.outcome!=='assigned'||!Number.isInteger(result.address)||result.address<0||result.address>63)throw new Error('Address assignment is unconfirmed or no free address remains. Completed changes remain saved.');
        await this.perform('discover_address',space,result.address);
      }
      if(full)this.ui.notice=`All 64 ${space} addresses are assigned. Additional unaddressed devices may remain.`;
      for(let address=0;address<64;address++){progress(`Refreshing ${space} · address ${address}`);await this.perform('discover_address',space,address);}
      const current=rows(this.ui.snapshot).filter(row=>row.bus.bus===this.bus&&(space==='outputs'?row.kind==='output':row.kind==='instance'));
      for(const row of current){this.assert();progress(`Reading ${row.item.name}`);await this.ui.read(row);this.assert();}
    }
    this.assert();this.ui.notice=this.ui.notice||'Reassignment, discovery and current-value refresh finished. Identify the new default names on this screen.';
  }
}
