import { Client, continuation } from './client.ts';
import { Draft, rows, itemGeneration, type Snapshot, type Row, type Bus, type Item } from './model.ts';

export class Controller {
  client=new Client();drafts=new Map<string,Draft>();hints=new Map<string,number[]>();
  error='';notice='';busy=0;progress='';cancelled=false;signingOut=false;
  private edits:Promise<void>=Promise.resolve();
  touch(){this.client.notify();}
  async logout(){
    if(this.signingOut)return;this.signingOut=true;this.cancelled=true;this.error='';this.notice='';this.touch();
    try{await this.client.logout();this.drafts.clear();}catch(error){this.error=(error as Error).message;}finally{this.signingOut=false;this.touch();}
  }
  get snapshot(){const value=this.client.snapshot;if(!value)throw new Error('Wait for the controller inventory.');return value;}
  get currentRows(){return rows(this.snapshot);}
  current(key:string){const row=this.currentRows.find(r=>r.key===key);if(!row)throw new Error('This fixture was removed or replaced.');return row;}
  async run(action:()=>Promise<void>){if(this.busy){this.error='Wait for the current operation to finish.';this.touch();return;}this.busy++;this.error='';this.notice='';this.touch();try{await action();}catch(error){this.error=(error as Error).message;}finally{this.busy--;this.touch();}}
  async changed(revision:number){return this.client.atRevision(revision);}
  async metadata(command:string,target:string|null,generation:string,params:Record<string,unknown>){
    const result=await this.client.job(generation,target,command,{...params,expected_revision:this.snapshot.inventory.revision});
    await this.changed(result.revision);return result;
  }
  rename(key:string,text:string,baseline:{target:string;generation:string}):Promise<void>{
    const operation=this.edits.catch(()=>{}).then(async()=>{
      if(this.progress)throw new Error('Stop the current batch before saving a name.');
      this.busy++;this.touch();
      try {
        const row=this.current(key);
        if(row.item.target!==baseline.target||row.generation!==baseline.generation)throw new Error('This device changed while you were editing. Discard the draft and review its name.');
        const result=await this.metadata('rename',baseline.target,baseline.generation,{name:text});
        const latest=this.current(key);
        if(latest.item.name!==text)throw new Error('The name changed again. Review the current device before retrying.');
        this.drafts.delete(key);this.notice='Name saved.';
        if(latest.kind==='output'||latest.kind==='instance'){
          try{await this.read(latest);}catch{this.notice='Name saved. Current values could not be refreshed; use Read current values.';}
        }
      } finally{this.busy--;this.touch();}
    });
    this.edits=operation;return operation;
  }
  async read(row:Row){const current=this.current(row.key);await this.client.job(current.generation,current.item.target,current.kind==='instance'?'read_input':'read_output',{});}
  async apply(row:Row,values:Record<string,unknown>){const current=this.current(row.key);await this.client.job(current.generation,current.item.target,'apply_output',{values});}
  async blink(row:Row){
    const current=this.current(row.key),active=this.snapshot.identification;
    if(active&&['starting','blinking','restoring'].includes(active.phase)&&active.target!==current.item.target)throw new Error('Stop the other blinking fixture first.');
    await this.client.job(current.generation,current.item.target,active?.target===current.item.target&&['starting','blinking','restoring'].includes(active.phase)?'stop_blink':'blink',{});
  }
  async configureBus(bus:number,name:string,enabled:boolean){const snapshot=this.snapshot;await this.metadata('configure_bus',null,itemGeneration(snapshot.inventory.inventory_epoch,snapshot.inventory.revision),{bus,name,enabled});this.notice='Bus settings saved.';}
  async remove(row:Row){const current=this.current(row.key);await this.metadata('remove_from_list',current.item.target,current.generation,{});this.notice='Removed from the saved list. Hardware addresses are unchanged.';}
  async representation(row:Row,value:string){const current=this.current(row.key);await this.metadata('set_color_representation',current.item.target,current.generation,{color_representation:value||null});await this.read(this.current(row.key));this.notice='Colour representation saved; actual values refreshed.';}
  private hint(report:any){const target=report.target;if(target){const row=this.currentRows.find(r=>r.item.target===target);if(row&&report.probe?.types_complete)this.hints.set(row.key,report.probe.device_types);}this.touch();}
  async scan(bus:Bus,assign:boolean){
    this.cancelled=false;let expected=this.snapshot.inventory.revision;const initial=this.snapshot;
    const perform=async(command:string,params:Record<string,unknown>)=>{
      if(this.cancelled)return null;
      if(!continuation(initial,this.snapshot,expected,bus.bus))throw new Error('Bus configuration changed during discovery. Review before starting again.');
      const result=await this.client.job(itemGeneration(initial.inventory.inventory_epoch,expected),null,command,{...params,expected_revision:expected});
      if(!Number.isSafeInteger(result.revision))throw new Error('The controller returned an invalid inventory revision.');
      const latest=await this.changed(result.revision);expected=result.revision;
      if(!continuation(initial,latest,expected,bus.bus))throw new Error('Configuration changed during discovery. Completed steps remain saved.');
      if(result.probe)this.hint(result);return result;
    };
    try {
      for(const space of ['outputs','inputs'])for(let address=0;address<64;address++){
        if(this.cancelled)break;this.progress=`Discovering ${space} · address ${address}`;this.touch();
        await perform('discover_address',{bus:bus.bus,space,address});
      }
      if(assign)for(const space of ['outputs','inputs'])for(let count=0;count<64&&!this.cancelled;count++){
        this.progress=`Finding unaddressed ${space}`;this.touch();
        const result=await perform(space==='outputs'?'assign_next_output':'assign_next_input',{bus:bus.bus});if(!result)break;
        if(result.outcome==='no_free_address')throw new Error(`No free ${space} addresses. Completed discovery remains saved.`);
        if(result.outcome===`no_unaddressed_${space==='outputs'?'output':'input'}`)break;
        if(result.outcome!=='assigned')throw new Error('An address is reserved but not confirmed. Review the saved item before continuing.');
        await perform('discover_address',{bus:bus.bus,space,address:result.address});
      }
      for(const row of this.currentRows.filter(r=>r.bus.bus===bus.bus&&(r.kind==='output'||r.kind==='instance'))){
        if(this.cancelled)break;
        if(!continuation(initial,this.snapshot,expected,bus.bus))throw new Error('Configuration changed before refresh.');
        this.progress=`Reading ${row.item.name}`;this.touch();await this.read(row);
      }
      this.notice=this.cancelled?'Stopped after the current operation. Completed changes remain saved.':'Discovery and current-value refresh finished.';
    }finally{this.progress='';this.touch();}
  }
  async createGroup(bus:Bus,group:number,name:string){const s=this.snapshot;await this.metadata('create_group',null,itemGeneration(s.inventory.inventory_epoch,s.inventory.revision),{bus:bus.bus,group,name});}
  async group(row:Row,group:number,member:boolean){const current=this.current(row.key);const result=await this.metadata('assign_group',current.item.target,current.generation,{group,member});if(result.outcome!=='verified')throw new Error('Saved assignment is not yet confirmed in hardware. Run Verify groups.');return result;}
  async readGroups(row:Row){const current=this.current(row.key);return this.client.job(current.generation,current.item.target,'read_groups',{});}
  async reviewGroups(row:Row,command:'adopt_groups'|'repair_groups',mask:number){const current=this.current(row.key);return this.metadata(command,current.item.target,current.generation,{reviewed_mask:mask});}
}
