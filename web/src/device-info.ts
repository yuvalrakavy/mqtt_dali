import type {Controller} from './controller.ts';
import {continuation} from './client.ts';
import {busReady,type PhysicalRow,type Snapshot} from './model.ts';

export type MemoryReading={kind:'value';raw:number}|{kind:'no_response'}|{kind:'receive_collision'|'transmit_collision';code:number|null};
export interface Identity {layout:'legacy'|'modern'|'unknown';gtin:string|null;serial_number:string|null;serial_bits:32|64|null;firmware_version:[number,number]|null;hardware_version:[number,number]|null;last_memory_bank:number|null}
export interface MemoryReport {operation:'read_identity'|'read_memory';outcome:'observed';target:string;target_generation:string;revision:number;bus:number;address:number;space:'outputs'|'inputs';bank:number;start:number;count:number;quality:'complete'|'partial'|'unavailable';version:MemoryReading|null;readings:{address:number;reading:MemoryReading|null}[];identity:Identity|null;reason:string|null;configuration_changed:false}
const byte=(v:unknown):v is number=>typeof v==='number'&&Number.isInteger(v)&&v>=0&&v<=255;
const invalid=():never=>{throw new Error('Device information is incomplete or invalid. Read again.');};
export function memoryWindow(bank:number,start:number,count:number){
  if(!byte(bank)||!byte(start)||!Number.isInteger(count)||count<1||count>64||start+count>255)throw new Error('Choose bank 0–255 and 1–64 bytes within locations 0–254.');
  return {bank,start,count};
}
function reading(v:any):MemoryReading|null{
  if(v===null)return null;
  if(!v||typeof v!=='object'||Array.isArray(v))return invalid();
  if(v.kind==='value'&&byte(v.raw)&&Object.keys(v).length===2)return v;
  if(v.kind==='no_response'&&Object.keys(v).length===1)return v;
  if(['receive_collision','transmit_collision'].includes(v.kind)&&(v.code===null||byte(v.code))&&Object.keys(v).length===2)return v;
  return invalid();
}
const numeric=(r:MemoryReading|null)=>r?.kind==='value'?r.raw:null;
export function decodeMemory(v:any,operation:'read_identity'|'read_memory',window?:{bank:number;start:number;count:number}):MemoryReport{
  if(!v||v.operation!==operation||v.outcome!=='observed'||v.configuration_changed!==false||!['complete','partial','unavailable'].includes(v.quality)||v.reason!==null&&typeof v.reason!=='string'||!Array.isArray(v.readings))return invalid();
  memoryWindow(v.bank,v.start,v.count);
  if(v.readings.length!==v.count)return invalid();
  const readings=v.readings.map((item:any,index:number)=>{
    if(!item||item.address!==v.start+index)return invalid();
    return {address:item.address,reading:reading(item.reading)};
  });
  const version=reading(v.version),bytes=readings.map((item:{reading:MemoryReading|null})=>numeric(item.reading));
  let quality=bytes.every((b:number|null)=>b!==null)?'complete':bytes.some((b:number|null)=>b!==null)?'partial':'unavailable';
  if(operation==='read_memory'){
    if(!window||v.bank!==window.bank||v.start!==window.start||v.count!==window.count||v.identity!==null||version!==null)return invalid();
  }else{
    if(v.bank!==0||v.start!==0||v.count>27)return invalid();
    const raw=numeric(version),layout=raw===1?'legacy':raw!==null&&raw>=8&&raw<=254?'modern':'unknown',last=layout==='legacy'?14:layout==='modern'?26:10;
    const header=bytes[0];
    if(header===null||header===255){
      if(v.count!==1||v.identity!==null)return invalid();
      if(header===255)quality='partial';
    }else{
      if(v.count!==Math.min(header,last)+1)return invalid();
      if(layout==='unknown'||header<last)quality='partial';
      const i=v.identity,bits=layout==='legacy'?32:layout==='modern'?64:null;
      if(!i||i.layout!==layout||i.serial_bits!==bits||i.gtin!==null&&(typeof i.gtin!=='string'||!/^\d{14}$/.test(i.gtin))||i.serial_number!==null&&(bits===null||typeof i.serial_number!=='string'||!new RegExp(`^[0-9A-F]{${bits/4}}$`).test(i.serial_number))||i.last_memory_bank!==null&&!byte(i.last_memory_bank))return invalid();
      for(const field of ['firmware_version','hardware_version'])if(i[field]!==null&&(!Array.isArray(i[field])||i[field].length!==2||!i[field].every(byte)))return invalid();
      if(layout!=='modern'&&i.hardware_version!==null)return invalid();
    }
  }
  if(v.quality!==quality)return invalid();
  return structuredClone(v);
}
export class DeviceInfoScope {
  readonly ui:Controller;readonly row:PhysicalRow;readonly initial:Snapshot;readonly epoch:number;
  cancelled=false;result:MemoryReport|null=null;
  constructor(ui:Controller,row:PhysicalRow){
    if(row.kind!=='output'&&row.kind!=='input')throw new Error('Select an individual fixture or input module.');
    this.ui=ui;this.row=structuredClone(row);this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;
  }
  assert(){
    const {ui,row}=this,current=ui.currentRows.find(r=>r.key===row.key);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Device information closed. Reopen to read again.');
    if(!continuation(this.initial,ui.snapshot,this.initial.inventory.revision,row.bus.bus)||!busReady(ui.snapshot,row.bus)||!current||current.kind!==row.kind||current.item.target!==row.item.target||current.generation!==row.generation||current.item.short_address!==row.item.short_address)throw new Error('Device configuration changed. Reopen device information.');
  }
  async read(window?:{bank:number;start:number;count:number}){
    this.result=null;this.assert();const frozen=window?memoryWindow(window.bank,window.start,window.count):undefined,operation=frozen?'read_memory':'read_identity';
    const r=await this.ui.client.job(this.row.generation,this.row.item.target,operation,frozen??{});
    this.assert();const row=this.row;
    if(!r||r.target!==row.item.target||r.target_generation!==row.generation||r.revision!==this.initial.inventory.revision||r.bus!==row.bus.bus||r.address!==row.item.short_address||r.space!==(row.kind==='output'?'outputs':'inputs'))return invalid();
    const result=decodeMemory(r,operation,frozen);this.result=result;return structuredClone(result);
  }
}
