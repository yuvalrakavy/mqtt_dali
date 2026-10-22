export interface Projection { target:string; binding_revision:number; location:{kind:string;bus:number;short_address?:number;group_address?:number;instance?:number;slot?:number}; members?:unknown[] }
export interface Ledger { baseline:Projection[]; batches:{revision:number;transitions:{from?:Projection|null;to?:Projection|null}[]}[] }
export interface IdentityItem { name:string;target:string;binding_revision:number;description?:string }
export interface Item extends IdentityItem { short_address:number;color_representation?:string;number?:number;instance_type?:number;instances?:Item[];group_address?:number;members?:number[] }
export interface SceneItem extends IdentityItem { slot:number }
export interface Bus {bus:number;name:string;enabled:boolean;outputs:Item[];inputs?:Item[];groups?:Item[];scenes?:SceneItem[]}
export interface Inventory {inventory_epoch:string;revision:number;name:string;description?:string;buses:Bus[];identity_ledger:Ledger}
export interface Snapshot {identity:{controller:string;version:string;build_utc:string};phase:string;hardware:{state:string;buses?:{bus:number;status:number}[]};inventory:Inventory;states:{target:string;generation:string;values:Record<string,unknown>}[];identification?:{target:string;generation:string;phase:string;actual_level:number|null}|null}
export interface Job {id:string;status:string;command?:string;target?:string|null;result?:any;error?:{code?:string;message?:string}|null}
export interface PhysicalRow {key:string;kind:'output'|'input'|'instance'|'group';bus:Bus;item:Item;parent?:Item;generation:string}
export interface SceneRow {key:string;kind:'scene';bus:Bus;item:SceneItem;generation:string}
export type Row=PhysicalRow|SceneRow;

export function itemGeneration(epoch:string,revision:number):string {
  if (!/^[a-f0-9]{32}$/.test(epoch)||!Number.isSafeInteger(revision)||revision<1) throw new Error('Invalid inventory authority; refresh the controller.');
  return `${epoch}.${revision}`;
}
export function rowOrigins(ledger:Ledger,epoch:string):Map<string,string> {
  const active=new Map<string,{key:string;record:Projection}>();
  const birth=(p:Projection)=>`${epoch}:${p.binding_revision}:${p.target}`;
  for (const p of ledger.baseline) {if(active.has(p.target))throw new Error('Duplicate inventory identity');active.set(p.target,{key:birth(p),record:p});}
  for (const batch of ledger.batches) {
    const continuing=new Map<number,string>();
    batch.transitions.forEach((edge,index)=>{
      if(!edge.from&&!edge.to)throw new Error('Empty inventory transition');
      if(edge.from) {
        const prior=active.get(edge.from.target);
        if(!prior||prior.record.binding_revision!==edge.from.binding_revision||JSON.stringify(prior.record.location)!==JSON.stringify(edge.from.location))throw new Error('Incomplete inventory history');
        continuing.set(index,prior.key);
      }
    });
    // Remove every source before inserting any destination: renames can swap names.
    for(const edge of batch.transitions)if(edge.from)active.delete(edge.from.target);
    batch.transitions.forEach((edge,index)=>{if(edge.to){if(active.has(edge.to.target))throw new Error('Conflicting inventory identity');active.set(edge.to.target,{key:continuing.get(index)??birth(edge.to),record:edge.to});}});
  }
  return new Map([...active].map(([target,value])=>[target,value.key]));
}
export function rows(snapshot:Snapshot):Row[] {
  const inventory=snapshot.inventory;
  itemGeneration(inventory.inventory_epoch,inventory.revision);
  const origins=rowOrigins(inventory.identity_ledger,inventory.inventory_epoch);
  const result:Row[]=[];
  for(const bus of inventory.buses) {
    const add=(kind:PhysicalRow['kind'],item:Item,parent?:Item)=>{
      const key=origins.get(item.target);if(!key)throw new Error('Saved device has no identity history');
      result.push({key,kind,bus,item,parent,generation:itemGeneration(inventory.inventory_epoch,item.binding_revision)});
    };
    for(const item of bus.outputs)add('output',item);
    for(const item of bus.groups??[])add('group',item);
    for(const item of bus.scenes??[]){const key=origins.get(item.target);if(!key)throw new Error('Saved scene has no identity history');result.push({key,kind:'scene',bus,item,generation:itemGeneration(inventory.inventory_epoch,item.binding_revision)});}
    for(const item of bus.inputs??[]){add('input',item);for(const instance of item.instances??[])add('instance',instance,item);}
  }
  return result;
}
export function glob(pattern:string,name:string):boolean {
  if(!pattern)return true;
  const escaped=pattern.replace(/[.+^${}()|[\]\\]/g,'\\$&').replace(/\*/g,'.*').replace(/\?/g,'.');
  return new RegExp(`^${escaped}$`,'iu').test(name);
}
export function busReady(snapshot:Snapshot,bus:Bus):boolean {
  return snapshot.phase==='ready'&&bus.enabled&&snapshot.hardware.state==='ready'&&!!snapshot.hardware.buses?.some(b=>b.bus===bus.bus&&b.status===2);
}
export function values(snapshot:Snapshot,row:Row):Record<string,unknown> {
  return snapshot.states.find(s=>s.target===row.item.target&&s.generation===row.generation)?.values??{};
}
export const inputType=(type?:number)=>(({1:'Part 301 · button',2:'Part 302 · input',3:'Part 303 · occupancy',4:'Part 304 · light sensor'} as Record<number,string>)[type??0]??`Type ${type??'unknown'}`);
export function inputDisplay(state:Record<string,unknown>):string {
  if(typeof state.Status==='boolean')return state.Status?'Pressed':'Released';
  if(typeof state.InputState==='boolean')return state.InputState?'On':'Off';
  if(typeof state.Occupied==='boolean')return state.Occupied?'Occupied':'Vacant';
  if(typeof state.InputValueRaw==='string')return `${typeof state.Position==='number'?`${Math.round(state.Position*100)}% · `:''}0x${state.InputValueRaw} / ${state.ValueResolution} bit`;
  if(typeof state.PositionRaw==='number')return `${state.PositionRaw} raw`;
  if(typeof state.LightLevelRaw==='number')return `${state.LightLevelRaw} raw`;
  return 'Unknown';
}

export class Draft {
  text:string;saved:string;error='';pending?:Promise<void>;
  baseline:{target:string;generation:string};
  constructor(name:string,target:string,generation:string){this.text=this.saved=name;this.baseline={target,generation};}
  get dirty(){return this.text!==this.saved;}
  commit(write:(text:string,baseline:{target:string;generation:string})=>Promise<void>):Promise<void> {
    if(this.pending)return this.pending;
    if(!this.dirty)return Promise.resolve();
    const text=this.text;
    if(!text.trim()){this.error='A name is required.';return Promise.reject(new Error(this.error));}
    this.error='';
    this.pending=Promise.resolve().then(()=>write(text,this.baseline)).then(()=>{this.saved=text;}).catch(error=>{this.error=error.message??'Name was not saved.';throw error;}).finally(()=>{this.pending=undefined;});
    return this.pending;
  }
}
export class Views {
  epoch=0;value:any=null;jobBook:Job[]=[];ready=false;
  begin(){this.epoch++;this.ready=false;return this.epoch;}
  snapshot(epoch:number,value:unknown){if(epoch===this.epoch){this.value=value;this.ready=true;}}
  jobs(epoch:number,book:Job[]){if(epoch===this.epoch)this.jobBook=book;}
  clear(){this.begin();this.value=null;this.jobBook=[];}
}
