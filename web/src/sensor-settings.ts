export const sensorKeys=['report','deadtime','hold','hysteresis','hysteresis_min'] as const;
export type SensorKey=typeof sensorKeys[number];
export type SensorPatch=Partial<Record<SensorKey,number>>;
export interface SensorValues {report:number|null;deadtime:number|null;hold:number|null;hold_supported:boolean|null;hysteresis:number|null;hysteresis_min:number|null;quality:'complete'|'partial'}
const byte=(v:unknown)=>typeof v==='number'&&Number.isInteger(v)&&v>=0&&v<=255;
export function applicableSensors(kind:number):readonly SensorKey[]{
  return kind===2?['report','deadtime']:kind===3?['report','deadtime','hold']:kind===4?['report','deadtime','hysteresis','hysteresis_min']:[];
}
function complete(v:SensorValues,kind:number){
  return byte(v.report)&&byte(v.deadtime)&&(kind===2||kind===3&&byte(v.hold)&&v.hold_supported===(v.hold!==255)
    ||kind===4&&byte(v.hysteresis)&&v.hysteresis!<=25&&byte(v.hysteresis_min));
}
export function decodeSensorSettings(raw:any,kind:number):SensorValues|null{
  if(raw===null||raw===undefined)return null;
  const keys=applicableSensors(kind);
  if(!keys.length||typeof raw!=='object'||Array.isArray(raw)||!['complete','partial'].includes(raw.quality)
    ||sensorKeys.some(k=>raw[k]!==null&&!byte(raw[k]))||sensorKeys.some(k=>!keys.includes(k)&&raw[k]!==null)
    ||(kind===3?raw.hold_supported!==(raw.hold===null?null:raw.hold!==255):raw.hold_supported!==null)
    ||raw.quality==='complete'&&!complete(raw,kind))throw new Error('Sensor settings evidence is incomplete or invalid. Inspect again.');
  return structuredClone(raw);
}
export function validateSensorPatch(patch:SensorPatch,v:SensorValues|null|undefined,kind:number){
  if(!v||v.quality!=='complete'||!complete(v,kind))throw new Error('Complete sensor settings evidence is required. Inspect again.');
  const keys=Object.keys(patch) as SensorKey[];
  if(!keys.length||keys.some(k=>!applicableSensors(kind).includes(k)))throw new Error('Choose known settings for this sensor type.');
  if(patch.hold!==undefined&&v.hold_supported!==true)throw new Error('This sensor does not support hold time.');
  if(keys.some(k=>!byte(patch[k])||k==='hold'&&patch[k]===255||k==='hysteresis'&&patch[k]!>25))throw new Error('Sensor values must use valid native steps and ranges. Hold time is 1s or a 10s increment.');
}
export function sensorMatches(patch:SensorPatch,v:SensorValues|null|undefined,kind:number){
  return !!v&&v.quality==='complete'&&complete(v,kind)&&sensorKeys.every(k=>patch[k]===undefined||patch[k]===v[k]&&(k!=='hold'||v.hold_supported===true));
}
export function sensorDisplay(key:SensorKey,raw:number){return key==='deadtime'?raw*50:key==='hold'?(raw===0?1:raw*10):raw;}
export function sensorRaw(key:SensorKey,display:string){
  if(display.trim()==='')return NaN;const n=Number(display);
  return key==='deadtime'?n/50:key==='hold'?(n===1?0:n>=10?n/10:NaN):n;
}
export function changedSensors(before:SensorValues,after:SensorValues):SensorPatch{
  const patch:SensorPatch={};for(const k of sensorKeys)if(before[k]!==after[k])patch[k]=after[k]??NaN;return patch;
}
