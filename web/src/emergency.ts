import type {Controller} from './controller.ts';
import {continuation} from './client.ts';
import {busReady,type PhysicalRow,type Snapshot} from './model.ts';
export interface EmergencyField {raw:number|null;status:'value'|'unknown'|'invalid'|'unavailable';value:number|null;unit:string;at_least:boolean}
export interface EmergencyValues {fields:Record<string,EmergencyField>;mode:Record<string,boolean|null>;features:Record<string,boolean|null>;failures:Record<string,boolean|null>;status:Record<string,boolean|null>;quality:'complete'|'partial'}
export type EmergencyPatch=Record<string,number>;
export const modeNames=['rest','normal','emergency','extended_emergency','function_test','duration_test','hardwired_inhibit','hardwired_switch_on'];
export const featureNames=['integral','maintained','switched_maintained','automatic_tests','adjustable_level','hardwired_inhibit','physical_selection','relight_in_rest'];
export const failureNames=['circuit_failure','battery_duration_failure','battery_failure','emergency_lamp_failure','function_max_delay_exceeded','duration_max_delay_exceeded','function_test_failed','duration_test_failed'];
export const statusNames=['inhibited','function_done_valid','duration_done_valid',null,'function_pending','duration_pending','identification_active','physically_selected'];
export const emergencySettings=[
  {key:'emergency_level',label:'Emergency level (native 1–254)',min:1,max:254,step:1},
  {key:'prolong_seconds',label:'Prolong (seconds)',min:0,max:7650,step:30},
  {key:'test_execution_timeout_minutes',label:'Pending timeout (minutes)',min:15,max:367200,step:1},
  {key:'function_test_interval_days',label:'Function interval (days)',min:0,max:255,step:1},
  {key:'function_delay_minutes',label:'First function test (minutes)',min:0,max:983010,step:15},
  {key:'duration_test_interval_weeks',label:'Duration interval (weeks)',min:0,max:97,step:1},
  {key:'duration_delay_minutes',label:'First duration test (minutes)',min:0,max:983010,step:15},
];
const integer=(value:unknown,min=0,max=255):value is number=>typeof value==='number'&&Number.isInteger(value)&&value>=min&&value<=max;
const object=(value:any)=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const invalid=():never=>{throw new Error('Emergency evidence is incomplete or inconsistent. Refresh to inspect again.');};
const equal=(a:any,b:any):boolean=>{
  if(a===b)return true;if(!object(a)||!object(b))return false;
  const keys=Object.keys(a).sort();return keys.join('\0')===Object.keys(b).sort().join('\0')&&keys.every(key=>equal(a[key],b[key]));
};
function reading(value:any):number|null{
  if(!object(value))return invalid();
  const keys=Object.keys(value).sort().join(',');
  if(value.kind==='value'&&keys==='kind,raw'&&integer(value.raw))return value.raw;
  if(value.kind==='no_response'&&keys==='kind')return null;
  if(['receive_collision','transmit_collision'].includes(value.kind)&&keys==='code,kind'&&(value.code===null||integer(value.code)))return null;
  return invalid();
}
function typeSupport(readings:any):boolean|null{
  const first=reading(readings.device_type);if(first===null||first===254)return null;if(first!==255)return first===1;
  let previous=-1,count=0;
  for(let index=0;index<9;index++){
    const value=reading(readings[`next_device_type_${index}`]);
    if(value===254)return count>=2?false:null;
    if(value===null||value===255||value<=previous)return null;
    if(value>=1)return value===1;previous=value;count++;
  }
  return null;
}
const definitions:[string,string,number,string][]=[
  ['battery_charge','raw',1,'battery'],['last_duration_seconds','seconds',120,'counter'],
  ['emergency_runtime_hours','hours',1,'counter'],['total_runtime_hours','hours',4,'counter'],
  ['emergency_level','raw',1,'level'],['minimum_level','raw',1,'level'],['maximum_level','raw',1,'level'],
  ['rated_duration_seconds','seconds',120,'counter'],['mode_raw','raw',1,'number'],['features_raw','raw',1,'number'],
  ['failures_raw','raw',1,'number'],['status_raw','raw',1,'number'],['extended_version','raw',1,'number'],
  ['function_test_interval_days','days',1,'number'],['duration_test_interval_weeks','weeks',1,'weeks'],
  ['test_execution_timeout_minutes','minutes',1440,'timeout'],['prolong_seconds','seconds',30,'number'],
];
function observed(readings:any):{support:boolean|null;observed:EmergencyValues|null}{
  if(!object(readings))return invalid();for(const value of Object.values(readings))reading(value);
  const support=typeSupport(readings),presence=reading(readings.presence);
  if(presence===null||support!==true)return {support,observed:null};
  const fields:Record<string,EmergencyField>={};
  for(const [key,unit,factor,kind]of definitions){
    const raw=reading(readings[key]);let status:EmergencyField['status']='value',value=raw===null?null:raw*factor;
    if(raw===null)status='unavailable';
    else if(raw===255&&['battery','level'].includes(kind)){status='unknown';value=null;}
    else if(kind==='level'&&raw===0||kind==='weeks'&&raw!==null&&raw>97){status='invalid';value=null;}
    else if(kind==='timeout'&&raw===0)value=15;
    fields[key]={raw,status,value,unit,at_least:kind==='counter'&&raw===255};
  }
  for(const key of ['next_function_test_minutes','next_duration_test_minutes']){
    const high=reading(readings[key+'_high']),low=reading(readings[key+'_low']),raw=high===null||low===null?null:high*256+low;
    fields[key]={raw,status:raw===null?'unavailable':raw===65535?'unknown':'value',value:raw===null||raw===65535?null:raw*15,unit:'minutes',at_least:false};
  }
  if(reading(readings.closing_presence)===null)return {support,observed:null};
  const bitmap=(key:string,names:(string|null)[])=>Object.fromEntries(names.flatMap((name,bit)=>name?[[name,fields[key].value===null?null:!!(fields[key].value!&(1<<bit))]]:[]));
  return {support,observed:{fields,mode:bitmap('mode_raw',modeNames),features:bitmap('features_raw',featureNames),failures:bitmap('failures_raw',failureNames),status:bitmap('status_raw',statusNames),quality:Object.values(readings).every(value=>reading(value)!==null)?'complete':'partial'}};
}
export function decodeEmergency(report:any):any{
  if(!object(report)||report.configuration_changed!==false||!object(report.readings))return invalid();
  const expected=observed(report.readings);if(report.support!==expected.support||!equal(report.observed,expected.observed))return invalid();
  if(report.before===null){if(report.before_readings!==null)return invalid();}
  else if(!object(report.before_readings)||!equal(report.before,observed(report.before_readings).observed))return invalid();
  return structuredClone(report);
}
export const emergencyValue=(values:EmergencyValues|null,key:string)=>values?.fields[key]?.status==='value'?values.fields[key].value:null;
export const settingValue=(values:EmergencyValues|null,key:string)=>emergencyValue(values,key==='function_delay_minutes'?'next_function_test_minutes':key==='duration_delay_minutes'?'next_duration_test_minutes':key);
export function emergencyWritable(key:string,values:EmergencyValues|null):boolean{
  if(!values||!emergencySettings.some(field=>field.key===key))return false;
  if(key==='emergency_level')return values.features.adjustable_level===true&&['emergency_level','minimum_level','maximum_level'].every(name=>emergencyValue(values,name)!==null)&&emergencyValue(values,'minimum_level')!<=emergencyValue(values,'maximum_level')!;
  if(key.includes('interval')||key.includes('delay'))return values.features.automatic_tests===true&&(key.includes('delay')||settingValue(values,key)!==null);
  return settingValue(values,key)!==null;
}
export function validateEmergencyPatch(patch:EmergencyPatch,values:EmergencyValues|null):void{
  if(!object(patch)||!Object.keys(patch).length)throw new Error('Change at least one emergency setting.');
  for(const [key,value]of Object.entries(patch)){
    const field=emergencySettings.find(field=>field.key===key);
    if(!field||!emergencyWritable(key,values)||!integer(value,field.min,field.max)||value%field.step!==0)throw new Error('Fresh supported emergency settings and valid native unit multiples are required.');
    if(key==='test_execution_timeout_minutes'&&value!==15&&(value<1440||value%1440!==0))throw new Error('Timeout must be 15 minutes or whole days.');
    if(key==='emergency_level'&&(value<emergencyValue(values,'minimum_level')!||value>emergencyValue(values,'maximum_level')!))throw new Error('Emergency level must meet the fresh manufacturer limits.');
  }
  for(const [interval,delay]of [['function_test_interval_days','function_delay_minutes'],['duration_test_interval_weeks','duration_delay_minutes']]){
    if((patch[interval]!==undefined&&patch[interval]>0)!==(patch[delay]!==undefined))throw new Error('A positive interval requires its explicit initial delay; delay-only and disabled pairs are invalid.');
  }
}
export class EmergencyScope {
  readonly ui:Controller;readonly row:PhysicalRow;readonly initial:Snapshot;readonly epoch:number;
  cancelled=false;result:any=null;values:EmergencyValues|null=null;
  constructor(ui:Controller,row:PhysicalRow){if(row.kind!=='output')throw new Error('Select an individual output.');this.ui=ui;this.row=structuredClone(row);this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;}
  assert(){
    const {ui,row}=this,current=ui.currentRows.find(current=>current.key===row.key);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Emergency operations stopped. Reopen to inspect again.');
    if(!continuation(this.initial,ui.snapshot,this.initial.inventory.revision,row.bus.bus)||!busReady(ui.snapshot,row.bus)||!current||current.kind!=='output'||current.item.target!==row.item.target||current.generation!==row.generation||current.item.short_address!==row.item.short_address)throw new Error('Output configuration changed. Reopen emergency lighting.');
  }
  identifying(){const state=this.ui.snapshot.identification;return state?.target===this.row.item.target&&['starting','blinking','restoring'].includes(state.phase);}
  private accept(report:any,operation:string){
    this.assert();const {row,initial}=this;
    if(!report||report.operation!==operation||report.target!==row.item.target||report.target_generation!==row.generation||report.revision!==initial.inventory.revision||report.bus!==row.bus.bus||report.address!==row.item.short_address)return invalid();
    const outcome=operation==='read_emergency'?'observed':operation==='configure_emergency'?'applied':'request_sent';
    if(report.outcome!==outcome)throw new Error('Emergency operation is unconfirmed. Refresh to inspect the fixture before trying again.');
    return decodeEmergency(report);
  }
  private async perform(command:string,params:Record<string,unknown>){
    this.assert();this.values=null;this.result=null;
    return this.accept(await this.ui.client.job(this.row.generation,this.row.item.target,command,params),command);
  }
  private retain(report:any){this.result=report;this.values=report.observed;return structuredClone(report);}
  async read(){return this.retain(await this.perform('read_emergency',{}));}
  async save(patch:EmergencyPatch){
    this.assert();if(this.identifying())throw new Error('Stop Blink before emergency operations.');
    const frozen=structuredClone(patch);validateEmergencyPatch(frozen,this.values);
    const report=await this.perform('configure_emergency',{expected_revision:this.initial.inventory.revision,settings:frozen});
    for(const [key,value]of Object.entries(frozen))if(settingValue(report.observed,key)!==value)throw new Error('Emergency settings do not match the requested readback. Refresh to inspect again.');
    return this.retain(report);
  }
  async test(test:'function'|'duration'|null){
    this.assert();if(this.identifying())throw new Error('Stop Blink before emergency operations.');
    if(!this.values||['mode_raw','features_raw','status_raw'].some(key=>emergencyValue(this.values,key)===null)||test!==null&&!['function','duration'].includes(test))throw new Error('Read supported emergency status before requesting a test.');
    return this.retain(await this.perform(test===null?'stop_emergency_test':'start_emergency_test',{expected_revision:this.initial.inventory.revision,...(test===null?{}:{test})}));
  }
}
