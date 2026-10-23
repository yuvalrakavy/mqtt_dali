import type { Controller } from './controller.ts';
import { continuation } from './client.ts';
import { busReady,type PhysicalRow,type Snapshot } from './model.ts';

export interface FeedbackValues {features:number[]|null;capability:number|null;supported:boolean|null;active:boolean|null;quality:'complete'|'partial'}
const byte=(v:unknown)=>typeof v==='number'&&Number.isInteger(v)&&v>=0&&v<=255;
export function decodeFeedback(raw:any):FeedbackValues{
  const invalid=()=>{throw new Error('Feedback evidence is incomplete or invalid. Refresh to inspect again.');};
  if(!raw||typeof raw!=='object'||Array.isArray(raw)||!['complete','partial'].includes(raw.quality))return invalid();
  const {features,capability,supported,active}=raw;
  if(features!==null&&(!Array.isArray(features)||features.length>65||features.some((v:unknown,i:number)=>!Number.isInteger(v)||typeof v!=='number'||v<32||v>96||i>0&&features[i-1]>=v)))return invalid();
  if(capability!==null&&!byte(capability)||supported!==null&&typeof supported!=='boolean'||active!==null&&typeof active!=='boolean')return invalid();
  let support:boolean|null=null,quality='partial';
  if(features===null){if(capability!==null||active!==null)return invalid();}
  else if(!features.includes(32)){support=false;quality='complete';if(capability!==null||active!==null)return invalid();}
  else if(capability===null){if(active!==null)return invalid();}
  else if(capability===0){support=false;quality='complete';if(active!==null)return invalid();}
  else if((capability&9)!==0){support=true;if(typeof active==='boolean')quality='complete';}
  else if(active!==null)return invalid();
  if(supported!==support||raw.quality!==quality)return invalid();
  return structuredClone(raw);
}
export function feedbackWritable(v:FeedbackValues|null){return !!v&&v.quality==='complete'&&v.supported===true&&v.features?.includes(32)&&typeof v.capability==='number'&&(v.capability&9)!==0&&typeof v.active==='boolean';}
export class FeedbackScope {
  readonly ui:Controller;readonly row:PhysicalRow;readonly initial:Snapshot;readonly epoch:number;
  cancelled=false;values:FeedbackValues|null=null;
  constructor(ui:Controller,row:PhysicalRow){
    if(row.kind!=='input'&&row.kind!=='instance')throw new Error('Select an input module or instance.');
    this.ui=ui;this.row=structuredClone(row);this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;
  }
  assert(){
    const {ui,row}=this,current=ui.currentRows.find(r=>r.key===row.key);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Feedback stopped. Reopen to inspect again.');
    if(!continuation(this.initial,ui.snapshot,this.initial.inventory.revision,row.bus.bus)||!busReady(ui.snapshot,row.bus)||!current||current.kind!==row.kind||current.item.target!==row.item.target||current.generation!==row.generation
      ||(row.kind==='input'?current.item.short_address!==row.item.short_address:current.parent?.short_address!==row.parent?.short_address||current.item.number!==row.item.number||current.item.instance_type!==row.item.instance_type))throw new Error('Input configuration changed. Reopen feedback.');
  }
  private accepted(r:any,operation:string):FeedbackValues|null{
    this.assert();const row=this.row,address=row.kind==='input'?row.item.short_address:row.parent?.short_address,instance=row.kind==='instance'?row.item.number:null;
    if(r.operation!==operation||r.target!==row.item.target||r.target_generation!==row.generation||r.revision!==this.initial.inventory.revision||r.bus!==row.bus.bus||r.address!==address||r.instance!==instance||r.configuration_changed!==false
      ||row.kind==='input'&&r.reported_type!==null||row.kind==='instance'&&r.feedback!==null&&r.reported_type!==row.item.instance_type)throw new Error('Feedback result changed. Refresh to inspect again.');
    const v=r.feedback===null?null:decodeFeedback(r.feedback);
    if(operation==='set_input_feedback'){
      if(r.outcome!=='set'||!feedbackWritable(v))throw new Error('Feedback was not confirmed. Refresh to inspect again.');
    }else if(r.outcome==='current'){
      if(!feedbackWritable(v))throw new Error('Current feedback evidence is incomplete.');
    }else if(r.outcome==='unsupported'){
      if(!v||v.quality!=='complete'||v.supported!==false)throw new Error('Unsupported feedback evidence is incomplete.');
    }else if(r.outcome==='unavailable'){
      if(v&&v.quality!=='partial')throw new Error('Unavailable feedback evidence is inconsistent.');
    }else if(r.outcome!=='type_mismatch'||v!==null)throw new Error('Feedback outcome is invalid.');
    return v;
  }
  async read(){
    this.assert();const r=await this.ui.client.job(this.row.generation,this.row.item.target,'read_input_feedback',{});
    const v=this.accepted(r,'read_input_feedback');this.values=v;return structuredClone(v);
  }
  async set(active:boolean){
    this.assert();if(typeof active!=='boolean'||!feedbackWritable(this.values))throw new Error('Complete supported feedback evidence is required. Refresh first.');
    const r=await this.ui.client.job(this.row.generation,this.row.item.target,'set_input_feedback',{expected_revision:this.initial.inventory.revision,active});
    const v=this.accepted(r,'set_input_feedback');
    if(v?.active!==active)throw new Error('Feedback does not match the requested state. Refresh to inspect again.');
    this.values=v;this.ui.notice='Feedback verified in hardware.';return structuredClone(v);
  }
}
