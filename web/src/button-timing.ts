export const timerKeys=['short','double','repeat','stuck'] as const;
export type TimerKey=typeof timerKeys[number];
export type ButtonPatch=Partial<Record<TimerKey,number>>;
export interface ButtonValues {short:number|null;short_min:number|null;double:number|null;double_min:number|null;repeat:number|null;stuck:number|null;quality:'complete'|'partial'}
const byte=(v:unknown)=>typeof v==='number'&&Number.isInteger(v)&&v>=0&&v<=255;
function complete(v:ButtonValues):boolean{
  return byte(v.short)&&byte(v.short_min)&&v.short_min!>=10&&v.short!>=v.short_min!
    &&byte(v.double)&&byte(v.double_min)&&v.double_min!>=10&&v.double_min!<=100
    &&(v.double===0||v.double!>=v.double_min!&&v.double!<=100)
    &&byte(v.repeat)&&v.repeat!>=5&&v.repeat!<=100&&byte(v.stuck)&&v.stuck!>=5;
}
export function decodeButtonTimers(raw:any):ButtonValues|null{
  if(raw===null||raw===undefined)return null;
  if(typeof raw!=='object'||Array.isArray(raw)||!['complete','partial'].includes(raw.quality)
    ||[...timerKeys,'short_min','double_min'].some(k=>raw[k]!==null&&!byte(raw[k]))
    ||raw.quality==='complete'&&!complete(raw))throw new Error('Button timing evidence is incomplete or invalid. Inspect again.');
  return structuredClone(raw);
}
export function validateButtonPatch(patch:ButtonPatch,before:ButtonValues|null|undefined){
  if(!before||before.quality!=='complete'||!complete(before))throw new Error('Complete button timing evidence is required. Inspect again.');
  if(!Object.keys(patch).length||Object.keys(patch).some(k=>!timerKeys.includes(k as TimerKey)))throw new Error('Change at least one known button timer.');
  const proposed={...before,...patch};
  if(!complete(proposed))throw new Error('Button timing must use whole native steps within the range and device minimum.');
}
export function timingMatches(patch:ButtonPatch,actual:ButtonValues|null|undefined){
  return !!actual&&actual.quality==='complete'&&complete(actual)&&timerKeys.every(k=>patch[k]===undefined||patch[k]===actual[k]);
}
export const timerDisplay=(key:TimerKey,raw:number)=>raw*(key==='stuck'?1:20);
// Preserve invalid fractional steps while editing; preflight refuses them before I/O.
export const timerRaw=(key:TimerKey,display:string)=>display.trim()===''?NaN:Number(display)/(key==='stuck'?1:20);
export function changedTimers(before:ButtonValues,after:ButtonValues):ButtonPatch{
  const patch:ButtonPatch={};for(const k of timerKeys)if(before[k]!==after[k])patch[k]=after[k]??NaN;return patch;
}
