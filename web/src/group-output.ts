import type { Controller } from './controller.ts';
import { continuation } from './client.ts';
import { busReady,type PhysicalRow,type Snapshot } from './model.ts';

export interface GroupValues {quality:'uniform'|'mixed'|'empty'|'partial'|'unavailable';power_state:boolean|null;brightness:number|null;power_mixed:boolean|null;brightness_mixed:boolean|null;known_members:number;unresolved:number[]}
export interface GroupResult {values:GroupValues;membership_matches:boolean|null}
const invalid=():never=>{throw new Error('Group evidence is incomplete or inconsistent. Refresh to inspect again.');};
const byte=(v:unknown)=>typeof v==='number'&&Number.isInteger(v)&&v>=0&&v<=255;
function reading(raw:any,mask=false):number|null {
  if(raw===null)return null;
  if(!raw||typeof raw!=='object'||Array.isArray(raw))return invalid();
  if(raw.kind==='value'){if(!byte(raw.raw)||mask&&raw.raw===255)return invalid();return raw.raw;}
  if(raw.kind==='mask'){if(!mask||raw.raw!==255)return invalid();return null;}
  if(raw.kind==='no_response')return null;
  if(['receive_collision','transmit_collision'].includes(raw.kind)&&(raw.code===null||byte(raw.code)))return null;
  return invalid();
}
export function groupIntent(patch:Record<string,unknown>){
  if(!patch||typeof patch!=='object'||Array.isArray(patch)||Object.keys(patch).some(k=>k!=='power'&&k!=='brightness')||!Object.keys(patch).length)throw new Error('Specify group power or a level.');
  if('power'in patch&&typeof patch.power!=='boolean'||'brightness'in patch&&(typeof patch.brightness!=='number'||!Number.isFinite(patch.brightness)||patch.brightness<0||patch.brightness>1))throw new Error('Group power must be boolean and level must be between 0 and 1.');
  if(patch.power===false&&typeof patch.brightness==='number'&&patch.brightness>0||patch.power===true&&patch.brightness===0)throw new Error('Group power and level contradict each other.');
  return structuredClone(patch);
}
export class GroupOutputScope {
  readonly ui:Controller;readonly row:PhysicalRow;readonly initial:Snapshot;readonly epoch:number;
  cancelled=false;result:GroupResult|null=null;
  constructor(ui:Controller,row:PhysicalRow){if(row.kind!=='group')throw new Error('Select a saved hardware group.');this.ui=ui;this.row=structuredClone(row);this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;}
  assert(){
    const {ui,row}=this,current=ui.currentRows.find(r=>r.key===row.key);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Group control stopped. Refresh to inspect again.');
    if(!continuation(this.initial,ui.snapshot,this.initial.inventory.revision,row.bus.bus)||!busReady(ui.snapshot,row.bus)||!current||current.kind!=='group'||current.item.target!==row.item.target||current.generation!==row.generation||current.item.group_address!==row.item.group_address)throw new Error('Group configuration changed. Refresh to inspect again.');
  }
  current(){try{this.assert();return true;}catch{return false;}}
  private accepted(r:any,operation:string):GroupResult {
    this.assert();const {row,initial}=this;
    if(!r||r.operation!==operation||r.outcome!==(operation==='read_group_output'?'observed':'sent')||r.target!==row.item.target||r.target_generation!==row.generation||r.revision!==initial.inventory.revision||r.bus!==row.bus.bus||r.group!==row.item.group_address||r.coverage!=='saved_inventory'||r.configuration_changed!==false)return invalid();
    const roster=this.initial.inventory.buses.find(b=>b.bus===row.bus.bus)!.outputs;
    if(!Array.isArray(r.members)||r.members.length!==roster.length||!r.values)return invalid();
    const seen=new Set<number>(),levels:number[]=[],unresolved:number[]=[];let answered=false,known=0,membershipComplete=true,membershipMatches=true;
    for(const m of r.members){
      const saved=roster.find(v=>v.short_address===m.address);
      if(!saved||seen.has(m.address)||m.target!==saved.target||m.target_generation!==`${initial.inventory.inventory_epoch}.${saved.binding_revision}`||m.expected_member!==(row.item.members?.includes(m.address)??false))return invalid();
      seen.add(m.address);const low=reading(m.membership_low),high=reading(m.membership_high),level=reading(m.actual_level,true),status=reading(m.status);
      answered ||=low!==null||high!==null||level!==null||status!==null||m.actual_level?.kind==='mask';
      const member=low===null||high===null?null:!!((low|(high<<8))&(1<<row.item.group_address!));
      if(m.member!==member)return invalid();membershipComplete&&=member!==null;membershipMatches&&=member===m.expected_member;
      if(member===null){unresolved.push(m.address);continue;}if(!member)continue;known++;
      if(level===null||status===null)unresolved.push(m.address);else levels.push(level);
    }
    let quality:GroupValues['quality'],power_state:boolean|null=null,brightness:number|null=null,power_mixed:boolean|null=null,brightness_mixed:boolean|null=null;
    if(unresolved.length)quality=answered?'partial':'unavailable';else if(!levels.length)quality='empty';else{
      power_mixed=levels.some(v=>(v>0)!==(levels[0]>0));brightness_mixed=levels.some(v=>v!==levels[0]);quality=brightness_mixed?'mixed':'uniform';
      if(!power_mixed)power_state=levels[0]>0;if(!brightness_mixed)brightness=levels[0]/254;
    }
    const v=r.values,expected={quality,power_state,brightness,power_mixed,brightness_mixed,known_members:known,unresolved};
    if(Object.entries(expected).some(([k,value])=>JSON.stringify(v[k])!==JSON.stringify(value))||r.membership_matches!==(membershipComplete?membershipMatches:null))return invalid();
    return {values:structuredClone(expected),membership_matches:r.membership_matches};
  }
  async read(){this.assert();this.result=null;const r=await this.ui.client.job(this.row.generation,this.row.item.target,'read_group_output',{});this.result=this.accepted(r,'read_group_output');return structuredClone(this.result);}
  async set(patch:Record<string,unknown>){this.assert();const intent=groupIntent(patch);this.result=null;
    const r=await this.ui.client.job(this.row.generation,this.row.item.target,'set_group_output',{expected_revision:this.initial.inventory.revision,...intent});
    this.result=this.accepted(r,'set_group_output');this.ui.notice='Group command sent. Shown values are sampled from saved fixtures.';return structuredClone(this.result);
  }
}
