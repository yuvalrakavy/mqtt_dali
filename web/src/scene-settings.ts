// Native scene values are distinct from live output intents. Null is MASK.
export type SceneLevel={kind:'mask'}|{kind:'value';value:number};
export type SceneColor={kind:'mask'}|{kind:'temperature';value:number|null}|{kind:'xy';x:number|null;y:number|null}|{kind:'rgbwaf';channels:(number|null)[];control:number|null};
export interface Setting {level:SceneLevel;color:SceneColor}
export const ignored=():Setting=>({level:{kind:'mask'},color:{kind:'mask'}});
const integer=(v:unknown,low:number,high:number)=>typeof v==='number'&&Number.isInteger(v)&&v>=low&&v<=high;
export function validSetting(s:any):s is Setting {
  if(!s||!s.level||!s.color)return false;
  if(s.level.kind!=='mask'&&!(s.level.kind==='value'&&integer(s.level.value,0,254)))return false;
  const c=s.color,nullable=(v:unknown,low:number,high:number)=>v===null||integer(v,low,high);
  return c.kind==='mask'||c.kind==='temperature'&&nullable(c.value,1,65534)||c.kind==='xy'&&nullable(c.x,0,65534)&&nullable(c.y,0,65534)||c.kind==='rgbwaf'&&Array.isArray(c.channels)&&c.channels.length===6&&c.channels.every((v:unknown)=>nullable(v,0,254))&&nullable(c.control,0,191);
}
export function summary(s:Setting|null):string {
  if(!s)return 'Unmanaged';
  const level=s.level.kind==='mask'?'Keep brightness':s.level.value===0?'Off':`${+(s.level.value*100/254).toFixed(1)}%`;
  const c=s.color;
  return c.kind==='mask'?s.level.kind==='mask'?'Leave unchanged':level:`${level} · ${c.kind==='temperature'?c.value===null?'Tc MASK':`${Math.round(1e6/c.value)} K`:c.kind==='xy'?'CIE xy':'RGBWAF'}`;
}
export class SettingDraft {
  readonly original:Setting|null;
  action:'ignore'|'off'|'brightness'|'keep';family:SceneColor['kind'];
  fields:Record<string,string>={};dirty=false;error='';pending?:Promise<void>;
  constructor(saved:Setting|null){
    this.original=structuredClone(saved);const s=saved??ignored();
    this.action=s.level.kind==='mask'?(s.color.kind==='mask'?'ignore':'keep'):s.level.value===0?'off':'brightness';this.family=s.color.kind;
  }
  chooseAction(value:SettingDraft['action']){this.action=value;this.dirty=true;this.error='';}
  chooseFamily(value:SceneColor['kind']){this.family=value;if(this.action==='ignore'&&value!=='mask')this.action='keep';this.fields={...this.fields};this.dirty=true;this.error='';}
  change(field:string,value:string){this.fields[field]=value;this.dirty=true;this.error='';}
  text(field:string):string {
    if(field in this.fields)return this.fields[field];
    const s=this.original??ignored(),c=s.color;
    if(field==='brightness')return s.level.kind==='value'?String(+(s.level.value*100/254).toFixed(3)):'100';
    if(field==='temperature')return c.kind==='temperature'&&c.value!==null?String(+(1e6/c.value).toFixed(3)):'';
    if(field==='x'||field==='y')return c.kind==='xy'&&c[field]!==null?String(c[field]!/65536):'';
    if(field==='control')return c.kind==='rgbwaf'&&c.control!==null?String(c.control):'';
    const index=channels.indexOf(field);return index>=0&&c.kind==='rgbwaf'&&c.channels[index]!==null?String(+(c.channels[index]!*100/254).toFixed(3)):'';
  }
  setting():Setting {
    const old=this.original??ignored();
    const number=(field:string,min:number,max:number,convert:(v:number)=>number,prior:number|null):number|null=>{
      if(!(field in this.fields))return prior;
      const text=this.fields[field].trim();if(text==='')return null;
      const value=Number(text);if(!Number.isFinite(value)||value<min||value>max)throw new Error(`Enter ${field} between ${min} and ${max}, or leave it blank for MASK.`);
      return convert(value);
    };
    if(this.action==='ignore')return ignored();
    let level:SceneLevel={kind:'mask'};
    if(this.action==='off')level={kind:'value',value:0};
    if(this.action==='brightness'){
      const raw=number('brightness',0,100,v=>Math.round(v*254/100),old.level.kind==='value'?old.level.value:254);
      if(raw===null)throw new Error('Brightness is required; choose Keep brightness for MASK.');level={kind:'value',value:raw};
    }
    let color:SceneColor={kind:'mask'};const c=old.color;
    if(this.family==='temperature')color={kind:'temperature',value:number('temperature',1e6/65534,1e6,v=>Math.round(1e6/v),c.kind==='temperature'?c.value:null)};
    if(this.family==='xy')color={kind:'xy',x:number('x',0,65534/65536,v=>Math.round(v*65536),c.kind==='xy'?c.x:null),y:number('y',0,65534/65536,v=>Math.round(v*65536),c.kind==='xy'?c.y:null)};
    if(this.family==='rgbwaf')color={kind:'rgbwaf',channels:channels.map((name,i)=>number(name,0,100,v=>Math.round(v*254/100),c.kind==='rgbwaf'?c.channels[i]:null)),control:number('control',128,191,v=>{if(!Number.isInteger(v))throw new Error('Control must be a whole native byte.');return v;},c.kind==='rgbwaf'?c.control:null)};
    const result={level,color};if(!validSetting(result))throw new Error('Scene values are invalid.');return result;
  }
  commit(write:(s:Setting)=>Promise<void>):Promise<void>{
    if(this.pending)return this.pending;if(!this.dirty)return Promise.resolve();
    this.error='';this.pending=Promise.resolve().then(()=>write(this.setting())).then(()=>{this.dirty=false;}).catch(error=>{this.error=error.message;throw error;}).finally(()=>{this.pending=undefined;});return this.pending;
  }
}
export const channels=['red','green','blue','white','amber','free'];
export function fromStored(stored:any):Setting|null {
  if(!stored||stored.quality!=='complete'||stored.stale)return null;
  try{
    const raw=(r:any,mask:number)=>{if(r?.kind==='mask'&&r.raw===mask)return null;if(r?.kind==='value'&&integer(r.raw,0,mask-1))return r.raw;throw new Error('Unresolved');};
    const levelRaw=raw(stored.level,255),level:SceneLevel=levelRaw===null?{kind:'mask'}:{kind:'value',value:levelRaw};
    let color:SceneColor={kind:'mask'},count=0;const attrs=stored.values;
    if(!attrs||typeof attrs!=='object')return null;
    if(stored.dt8===false){if(stored.color_type!==null)return null;}
    else if(stored.dt8===true){const type=raw(stored.color_type,255);
      if(type===0x20){color={kind:'temperature',value:raw(attrs.temperature,65535)};count=1;}
      else if(type===0x10){color={kind:'xy',x:raw(attrs.x,65535),y:raw(attrs.y,65535)};count=2;}
      else if(type===0x80){color={kind:'rgbwaf',channels:channels.map(c=>raw(attrs[c],255)),control:raw(attrs.control,255)};count=7;if(color.control!==null&&color.control<128)return null;}
      else if(type!==null)return null;
    }else return null;
    const result={level,color};return Object.keys(attrs).length===count&&validSetting(result)?result:null;
  }catch{return null;}
}
