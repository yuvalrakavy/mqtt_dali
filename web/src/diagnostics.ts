import type {MemoryReading} from './device-info.ts';
export type DiagnosticBank=205|206|207;
export interface DiagnosticField {status:'value'|'temporarily_unavailable'|'read_protected'|'unknown'|'unavailable'|'invalid';raw:number|null;value:string|boolean|null;unit:string|null}
export interface DiagnosticData {version:number|null;format_status:'supported'|'unrecognized'|'unavailable';fields:Record<string,DiagnosticField>;snapshot:{start:MemoryReading;end:MemoryReading;release:MemoryReading}|null}
export function diagnosticBank(value:number):DiagnosticBank {if(value!==205&&value!==206&&value!==207)throw new Error('Choose diagnostic bank205,206 or207.');return value;}
type Mask='invalid'|'read_protected'|'unknown';
interface Definition {key:string;label:string;start:number;width:number;unit:string|null;scale:number;maximum:number;mask:Mask;kind:'number'|'boolean'|'temperature'}
const number=(key:string,label:string,start:number,width:number,unit:string|null='count',scale=0,mask:Mask='invalid',maximum=2**(width*8)-3):Definition=>({key,label,start,width,unit,scale,maximum,mask,kind:'number'});
const flag=(key:string,label:string,start:number,mask:Mask='read_protected'):Definition=>({...number(key,label,start,1,null,0,mask,1),kind:'boolean'});
const temperature=(key:string,label:string,start:number,mask:Mask):Definition=>({...number(key,label,start,1,'°C',0,mask),kind:'temperature'});
const pair=(key:string,label:string,start:number)=>[flag(key,label,start),number(`${key}_counter`,`${label} count`,start+1,1,'count',0,'read_protected')];
const definitions:Record<DiagnosticBank,Definition[]>={
 205:[number('operating_time','Driver operating time',4,4,'s'),number('start_counter','Driver starts',8,3),number('supply_voltage','Supply voltage',11,2,'Vrms',-1,'read_protected'),number('supply_frequency','Supply frequency',13,1,'Hz',0,'read_protected'),number('power_factor','Power factor',14,1,null,-2,'read_protected',100),flag('overall_failure','Driver failure',15,'invalid'),number('overall_failure_counter','Driver failure count',16,1),...pair('undervoltage','Undervoltage',17),...pair('overvoltage','Overvoltage',19),...pair('power_limit','Power limitation',21),...pair('thermal_derating','Thermal derating',23),...pair('thermal_shutdown','Thermal shutdown',25),temperature('temperature','Driver temperature',27,'invalid'),number('output_current_percent','Output current',28,1,'%',0,'invalid',100)],
 206:[number('resettable_start_counter','Resettable light-source starts',4,3),number('start_counter','Light-source starts',7,3),number('resettable_on_time','Resettable on time',10,4,'s'),number('on_time','Light-source on time',14,4,'s'),number('voltage','Light-source voltage',18,2,'V',-1),number('current','Light-source current',20,2,'A',-3),flag('overall_failure','Light-source failure',22,'invalid'),number('overall_failure_counter','Light-source failure count',23,1),...pair('short_circuit','Short circuit',24),...pair('open_circuit','Open circuit',26),...pair('thermal_derating','Thermal derating',28),...pair('thermal_shutdown','Thermal shutdown',30),temperature('temperature','Light-source temperature',32,'read_protected')],
 207:[number('rated_life','Rated median useful life',4,1,'h',3,'unknown'),temperature('reference_temperature','Reference driver temperature',5,'unknown'),number('rated_starts','Rated light-source starts',6,2,'count',2,'unknown')]
};
export const diagnosticFields=(bank:DiagnosticBank)=>definitions[bank].map(({key,label})=>({key,label}));
const invalid=():never=>{throw new Error('Diagnostic data is incomplete or invalid. Read again.');};
function decimal(raw:number,scale:number):string{
 const value=BigInt(raw);if(scale>=0)return(value*10n**BigInt(scale)).toString();
 const digits=-scale,divisor=10n**BigInt(digits),fraction=value%divisor;
 return fraction===0n?(value/divisor).toString():`${value/divisor}.${fraction.toString().padStart(digits,'0').replace(/0+$/,'')}`;
}
function field(d:Definition,bytes:(number|null)[]):DiagnosticField{
 const values=bytes.slice(d.start,d.start+d.width),raw=values.length===d.width&&values.every(v=>v!==null)?values.reduce<number>((n,b)=>n*256+b!,0):null;
 const max=2**(d.width*8)-1,status=raw===null?'unavailable':raw===max-1?'temporarily_unavailable':raw===max?d.mask:raw>d.maximum?'invalid':'value';
 const value=status!=='value'?null:d.kind==='boolean'?raw===1:d.kind==='temperature'?String(raw!-60):decimal(raw!,d.scale);
 return{status,raw,value,unit:d.unit};
}
export function decodeDiagnostics(raw:any,bank:DiagnosticBank,bytes:(number|null)[]):{data:DiagnosticData;partial:boolean}{
 diagnosticBank(bank);const last=bank===205?28:bank===206?32:7;
 if(!raw||!raw.fields||typeof raw.fields!=='object'||Array.isArray(raw.fields)||bytes.length<1||bytes.length>last+1||bytes.some(v=>v!==null&&(!Number.isInteger(v)||v<0||v>255)))return invalid();
 const header=bytes[0];if(header===null||header===255){
  if(bytes.length!==1||raw.version!==null||raw.format_status!=='unavailable'||Object.keys(raw.fields).length!==0||raw.snapshot!==null)return invalid();
  return{data:structuredClone(raw),partial:header===255};
 }
 const version=bytes[3]??null,status=version===1?'supported':version===null?'unavailable':'unrecognized';
 if(raw.version!==version||raw.format_status!==status||bytes.length!==Math.min(header,version===1?last:3)+1)return invalid();
 if(version===1){
  const defs=definitions[bank];if(Object.keys(raw.fields).length!==defs.length)return invalid();
  for(const d of defs){const actual=raw.fields[d.key],expected=field(d,bytes);if(!actual||actual.status!==expected.status||actual.raw!==expected.raw||actual.value!==expected.value||actual.unit!==expected.unit)return invalid();}
  if(bank!==207){const proof=raw.snapshot;if(!proof||Object.keys(proof).length!==3)return invalid();for(const[key,value]of[['start',170],['end',170],['release',255]]as const){const r=proof[key];if(!r||r.kind!=='value'||r.raw!==value||Object.keys(r).length!==2)return invalid();}}
  else if(raw.snapshot!==null)return invalid();
 }else if(Object.keys(raw.fields).length!==0||raw.snapshot!==null)return invalid();
 return{data:structuredClone(raw),partial:version!==1||header<last};
}
export function diagnosticDisplay(field:DiagnosticField):string {
 if(field.status==='value')return typeof field.value==='boolean'?(field.value?'Yes':'No'):`${field.value}${field.unit?' '+field.unit:''}`;
 return({temporarily_unavailable:'Temporarily unavailable',read_protected:'Read protected',unknown:'Unknown',unavailable:'Unavailable',invalid:'Invalid data'})[field.status];
}
