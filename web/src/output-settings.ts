import type {Controller} from './controller.ts';
import {continuation} from './client.ts';
import {busReady,type PhysicalRow,type Snapshot} from './model.ts';
export interface OutputValues {
  physical_minimum:number|null;minimum_level:number|null;maximum_level:number|null;
  power_on_level:number|null;system_failure_level:number|null;fade_raw:number|null;
  fade_time:number|null;fade_rate:number|null;quality:'complete'|'partial';
}
export const outputFields=['minimum_level','maximum_level','power_on_level','system_failure_level','fade_time','fade_rate'] as const;
export type OutputField=typeof outputFields[number];
export type OutputPatch=Partial<Record<OutputField,number>>;
export interface OutputResult {settings:OutputValues|null;status:number|null;actual_level:number|null;version:number|null}
const invalid=():never=>{throw new Error('Output settings evidence is incomplete or inconsistent. Refresh to inspect again.');};
const integer=(v:unknown,min:number,max:number):v is number=>typeof v==='number'&&Number.isInteger(v)&&v>=min&&v<=max;
export function levelPercent(raw:number){return (raw*100/254).toFixed(1);}
export function percentLevel(text:string){const value=text.trim()===''?NaN:Number(text);return Number.isFinite(value)&&value>=0&&value<=100?Math.round(value*254/100):NaN;}
export const fadeTimes=Array.from({length:16},(_,code)=>({code,label:code===0?'Use other fade settings (code 0)':`${(0.5*2**(code/2)).toFixed(code<4?2:1)} s`}));
export const fadeRates=Array.from({length:15},(_,n)=>({code:n+1,label:`${(506/2**((n+1)/2)).toFixed(1)} steps/s`}));
export function decodeOutputValues(raw:any):OutputValues{
  if(!raw||typeof raw!=='object'||Array.isArray(raw))return invalid();
  for(const field of ['physical_minimum','minimum_level','maximum_level'])if(raw[field]!==null&&!integer(raw[field],1,254))return invalid();
  for(const field of ['power_on_level','system_failure_level','fade_raw'])if(raw[field]!==null&&!integer(raw[field],0,255))return invalid();
  const time=raw.fade_raw===null?null:raw.fade_raw>>4,rate=raw.fade_raw===null||!(raw.fade_raw&15)?null:raw.fade_raw&15;
  if(raw.fade_time!==time||raw.fade_rate!==rate)return invalid();
  const complete=raw.physical_minimum!==null&&raw.minimum_level!==null&&raw.maximum_level!==null&&raw.physical_minimum<=raw.minimum_level&&raw.minimum_level<=raw.maximum_level&&raw.power_on_level!==null&&raw.system_failure_level!==null&&raw.fade_raw!==null&&rate!==null;
  if(raw.quality!==(complete?'complete':'partial'))return invalid();
  return structuredClone(raw);
}
export function outputWritable(field:OutputField,v:OutputValues|null):boolean{
  if(!v)return false;
  if(field==='minimum_level'||field==='maximum_level')return v.physical_minimum!==null&&v.minimum_level!==null&&v.maximum_level!==null&&v.physical_minimum<=v.minimum_level&&v.minimum_level<=v.maximum_level;
  if(field==='fade_time'||field==='fade_rate')return v.fade_raw!==null;
  return v[field]!==null;
}
export function validateOutputPatch(patch:OutputPatch,v:OutputValues|null){
  if(!patch||typeof patch!=='object'||Array.isArray(patch)||!Object.keys(patch).length)throw new Error('Change at least one output setting.');
  for(const [key,value] of Object.entries(patch)){
    if(!outputFields.includes(key as OutputField)||!outputWritable(key as OutputField,v))throw new Error('Fresh supported setting evidence is required. Refresh first.');
    const min=key==='minimum_level'||key==='maximum_level'||key==='fade_rate'?1:0,max=key==='minimum_level'||key==='maximum_level'?254:key==='fade_time'||key==='fade_rate'?15:255;
    if(!integer(value,min,max))throw new Error('Enter valid brightness levels and native fade choices.');
  }
  if(patch.minimum_level!==undefined||patch.maximum_level!==undefined){
    const min=patch.minimum_level??v!.minimum_level!,max=patch.maximum_level??v!.maximum_level!;
    if(min<v!.physical_minimum!||min>max)throw new Error('Minimum must meet the physical minimum and not exceed maximum.');
  }
}
export function changedOutputSettings(before:OutputValues,after:OutputValues):OutputPatch{
  const patch:OutputPatch={};for(const field of outputFields)if(before[field]!==after[field])Object.assign(patch,{[field]:after[field]});return patch;
}
function reading(raw:any,mask=false):number|null{
  if(!raw||typeof raw!=='object'||Array.isArray(raw))return invalid();
  if(raw.kind==='value'){if(!integer(raw.raw,0,255)||mask&&raw.raw===255)return invalid();return raw.raw;}
  if(raw.kind==='mask'){if(!mask||raw.raw!==255)return invalid();return null;}
  if(raw.kind==='no_response')return null;
  if(['receive_collision','transmit_collision'].includes(raw.kind)&&(raw.code===null||integer(raw.code,0,255)))return null;
  return invalid();
}
export class OutputSettingsScope {
  readonly ui:Controller;readonly row:PhysicalRow;readonly initial:Snapshot;readonly epoch:number;
  cancelled=false;values:OutputValues|null=null;result:OutputResult|null=null;
  constructor(ui:Controller,row:PhysicalRow){if(row.kind!=='output')throw new Error('Select an individual output.');this.ui=ui;this.row=structuredClone(row);this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;}
  assert(){
    const {ui,row}=this,current=ui.currentRows.find(r=>r.key===row.key);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Output settings stopped. Reopen to inspect again.');
    if(!continuation(this.initial,ui.snapshot,this.initial.inventory.revision,row.bus.bus)||!busReady(ui.snapshot,row.bus)||!current||current.kind!=='output'||current.item.target!==row.item.target||current.generation!==row.generation||current.item.short_address!==row.item.short_address)throw new Error('Output configuration changed. Reopen settings.');
  }
  identifying(){const s=this.ui.snapshot.identification;return s?.target===this.row.item.target&&['starting','blinking','restoring'].includes(s.phase);}
  private accepted(r:any,operation:string):OutputResult{
    this.assert();const {row,initial}=this;
    if(!r||r.operation!==operation||r.target!==row.item.target||r.target_generation!==row.generation||r.revision!==initial.inventory.revision||r.bus!==row.bus.bus||r.address!==row.item.short_address||r.configuration_changed!==false)return invalid();
    if(!r.readings||typeof r.readings!=='object'||Array.isArray(r.readings))return invalid();
    const raw:Record<string,number|null>={};
    for(const key of ['status','version','actual_level','physical_minimum','minimum_level','maximum_level','power_on_level','system_failure_level','fade_time_rate','closing_status'])raw[key]=reading(r.readings[key],['actual_level','physical_minimum','minimum_level','maximum_level'].includes(key));
    const present=raw.status!==null&&raw.closing_status!==null,v=r.settings===null?null:decodeOutputValues(r.settings);
    if((v!==null)!==present)return invalid();
    if(v){
      for(const key of ['physical_minimum','minimum_level','maximum_level','power_on_level','system_failure_level'] as const){const expected=key==='power_on_level'||key==='system_failure_level'?raw[key]:integer(raw[key],1,254)?raw[key]:null;if(v[key]!==expected)return invalid();}
      if(v.fade_raw!==raw.fade_time_rate)return invalid();
    }
    if(operation==='configure_output'){if(r.outcome!=='configured'||v===null)throw new Error('Output settings were not confirmed. Refresh to inspect again.');}
    else if(r.outcome!==(v===null?'unavailable':v.quality==='complete'?'current':'partial'))return invalid();
    return {settings:v,status:present?raw.closing_status:null,actual_level:raw.actual_level,version:raw.version};
  }
  async read(){this.assert();this.values=null;this.result=null;const r=await this.ui.client.job(this.row.generation,this.row.item.target,'read_output_settings',{});const result=this.accepted(r,'read_output_settings');this.values=result.settings;this.result=result;return structuredClone(result);}
  async save(patch:OutputPatch){
    this.assert();if(this.identifying())throw new Error('Stop Blink before changing fixture settings.');
    const frozen=structuredClone(patch);validateOutputPatch(frozen,this.values);this.values=null;this.result=null;
    const r=await this.ui.client.job(this.row.generation,this.row.item.target,'configure_output',{expected_revision:this.initial.inventory.revision,settings:frozen});
    const result=this.accepted(r,'configure_output');
    for(const field of outputFields)if(frozen[field]!==undefined&&result.settings?.[field]!==frozen[field])throw new Error('Output settings do not match requested values. Refresh to inspect again.');
    this.values=result.settings;this.result=result;this.ui.notice='Fixture settings verified in hardware.';return structuredClone(result);
  }
}
