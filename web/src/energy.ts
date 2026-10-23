import type {MemoryReading} from './device-info.ts';
export type EnergyBank=202|203|204;
export interface EnergyField {status:'value'|'temporarily_unavailable'|'unavailable'|'invalid';raw:number|null;scale:number|null;value:string|null;unit:string}
export interface EnergyData {version:number|null;format_status:'supported'|'unrecognized'|'unavailable';fields:Record<string,EnergyField>;snapshot:{start:MemoryReading;end:MemoryReading;release:MemoryReading}|null}
export function energyBank(value:number):EnergyBank {
  if(value!==202&&value!==203&&value!==204)throw new Error('Choose energy bank 202, 203 or 204.');return value;
}
const invalid=():never=>{throw new Error('Energy data is incomplete or invalid. Read again.');};
const decimal=(raw:number,scale:number)=>{
  const value=BigInt(raw);if(scale>=0)return (value*10n**BigInt(scale)).toString();
  const digits=-scale,divisor=10n**BigInt(digits),fraction=value%divisor;
  return fraction===0n?(value/divisor).toString():`${value/divisor}.${fraction.toString().padStart(digits,'0').replace(/0+$/,'')}`;
};
function field(f:any,bytes:(number|null)[],start:number,width:number,unit:string){
  const rawScale=bytes[start]??null,scale=rawScale===null?null:rawScale>=128?rawScale-256:rawScale,values=bytes.slice(start+1,start+width+1);
  const raw=values.length===width&&values.every(v=>v!==null)?values.reduce<number>((total,v)=>total*256+v!,0):null,max=2**(width*8)-1;
  const status=scale!==null&&(scale< -6||scale>6)?'invalid':scale===null||raw===null?'unavailable':raw===max-1?'temporarily_unavailable':raw===max?'invalid':'value';
  if(!f||f.status!==status||f.raw!==raw||f.scale!==scale||f.unit!==unit||f.value!==(status==='value'?decimal(raw!,scale!):null))return invalid();
}
export function decodeEnergy(raw:any,bank:EnergyBank,bytes:(number|null)[]):{data:EnergyData;partial:boolean}{
  if(!raw||!raw.fields||typeof raw.fields!=='object'||Array.isArray(raw.fields)||bytes.length<1||bytes.length>16)return invalid();
  const header=bytes[0];
  if(header===null||header===255){
    if(bytes.length!==1||raw.version!==null||raw.format_status!=='unavailable'||Object.keys(raw.fields).length!==0||raw.snapshot!==null)return invalid();
    return {data:structuredClone(raw),partial:header===255};
  }
  const version=bytes[3]??null,status=version===1?'supported':version===null?'unavailable':'unrecognized';
  if(raw.version!==version||raw.format_status!==status||bytes.length!==Math.min(header,version===1?15:3)+1)return invalid();
  if(version===1){
    if(Object.keys(raw.fields).length!==2)return invalid();
    field(raw.fields.energy,bytes,4,6,bank===203?'VAh':'Wh');field(raw.fields.power,bytes,11,4,bank===203?'VA':'W');
    const proof=raw.snapshot;if(!proof||Object.keys(proof).length!==3)return invalid();
    for(const [key,value]of [['start',170],['end',170],['release',255]] as const){const reading=proof[key];if(!reading||reading.kind!=='value'||reading.raw!==value||Object.keys(reading).length!==2)return invalid();}
  }else if(Object.keys(raw.fields).length!==0||raw.snapshot!==null)return invalid();
  return {data:structuredClone(raw),partial:version!==1||header<15};
}
export function energyDisplay(field:EnergyField):string {
  return field.status==='value'?`${field.value} ${field.unit}`:field.status==='temporarily_unavailable'?'Temporarily unavailable':field.status==='invalid'?'Invalid data':'Unavailable';
}
