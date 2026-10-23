import type { Controller } from './controller.ts';
import { continuation } from './client.ts';
import { busReady, type PhysicalRow, type Snapshot } from './model.ts';

export interface InputValues {
  enabled:boolean;event_priority:number;event_scheme:number;event_filter:number;
  filter_bytes:(number|null)[];filter_width:number;groups:number[];quality:string;
}
export interface InputPatch {enabled?:boolean;event_priority?:number;event_scheme?:number;event_filter?:number;groups?:(number|null)[]}
export const eventFlags:Record<number,string[]>={
  1:['Released','Pressed','Short press','Double press','Long press start','Long press repeat','Long press stop','Stuck / free'],
  2:['Position changed'],3:['Occupied','Vacant','Repeat','Movement','No movement'],4:['Light level changed'],
};
export class InputSettingsScope {
  readonly ui:Controller;readonly row:PhysicalRow;readonly initial:Snapshot;readonly epoch:number;
  cancelled=false;values:InputValues|null=null;
  constructor(ui:Controller,row:PhysicalRow){
    if(row.kind!=='instance')throw new Error('Select an input instance.');
    this.ui=ui;this.row=structuredClone(row);this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;
  }
  assert(){
    const ui=this.ui,current=ui.currentRows.find(r=>r.key===this.row.key);
    if(this.cancelled||ui.cancelled||ui.signingOut||!ui.client.views.ready||ui.client.views.epoch!==this.epoch)throw new Error('Input settings stopped. Reopen to inspect current settings.');
    if(!continuation(this.initial,ui.snapshot,this.initial.inventory.revision,this.row.bus.bus)||!busReady(ui.snapshot,this.row.bus)||!current||current.kind!=='instance'||current.item.target!==this.row.item.target||current.generation!==this.row.generation||current.parent?.short_address!==this.row.parent?.short_address||current.item.number!==this.row.item.number||current.item.instance_type!==this.row.item.instance_type)
      throw new Error('Input configuration changed. Reopen settings.');
  }
  private accepted(result:any,operation:string):InputValues {
    this.assert();const row=this.row;
    if(result.operation!==operation||result.target!==row.item.target||result.target_generation!==row.generation||result.revision!==this.initial.inventory.revision||result.bus!==row.bus.bus||result.address!==row.parent?.short_address||result.instance!==row.item.number||result.reported_type!==row.item.instance_type||result.configuration_changed!==false)
      throw new Error('Input settings result changed. Inspect again.');
    if(result.outcome!==(operation==='configure_input'?'configured':'current'))throw new Error('Input settings are unavailable or unconfirmed. Inspect again.');
    const v=result.settings,mask=({1:255,2:1,3:31,4:1} as Record<number,number>)[row.item.instance_type??-1];
    if(!v||v.quality!=='complete'||typeof v.enabled!=='boolean'||!Number.isInteger(v.event_priority)||v.event_priority<2||v.event_priority>5||!Number.isInteger(v.event_scheme)||v.event_scheme<0||v.event_scheme>4||!Number.isInteger(v.event_filter)||v.event_filter<0||v.event_filter>0xffffff||mask!==undefined&&(v.event_filter&~mask)!==0||!Array.isArray(v.groups)||v.groups.length!==3||v.groups.some((g:unknown)=>!Number.isInteger(g)||typeof g!=='number'||g<0||g>31&&g!==255)||v.filter_width!==(mask===undefined?24:8))
      throw new Error('Input settings evidence is incomplete. Inspect again.');
    if(!Array.isArray(v.filter_bytes)||v.filter_bytes.length!==3||v.filter_bytes.some((b:unknown,i:number)=>i<v.filter_width/8?b!==((v.event_filter>>>(8*i))&255):b!==null))throw new Error('Input filter evidence is incomplete. Inspect again.');
    return structuredClone(v);
  }
  async read(){
    this.assert();const r=await this.ui.client.job(this.row.generation,this.row.item.target,'read_input_settings',{});
    const values=this.accepted(r,'read_input_settings');this.values=values;return structuredClone(values);
  }
  async save(patch:InputPatch){
    this.assert();if(!this.values||!Object.keys(patch).length)throw new Error('Change at least one input setting.');
    const frozen=structuredClone(patch),result=await this.ui.client.job(this.row.generation,this.row.item.target,'configure_input',{expected_revision:this.initial.inventory.revision,settings:frozen});
    const actual=this.accepted(result,'configure_input');
    for(const field of ['enabled','event_priority','event_scheme','event_filter'] as const)if(frozen[field]!==undefined&&frozen[field]!==actual[field])throw new Error('Input settings do not match the requested values. Inspect again.');
    if(frozen.groups&&JSON.stringify(frozen.groups.map(g=>g??255))!==JSON.stringify(actual.groups))throw new Error('Input groups do not match the requested values. Inspect again.');
    this.values=actual;this.ui.notice='Input event settings verified in hardware.';return structuredClone(actual);
  }
}

export function changedSettings(before:InputValues,after:InputValues):InputPatch{
  const patch:InputPatch={};
  for(const field of ['enabled','event_priority','event_scheme','event_filter'] as const)if(before[field]!==after[field])Object.assign(patch,{[field]:after[field]});
  if(JSON.stringify(before.groups)!==JSON.stringify(after.groups))patch.groups=after.groups.map(g=>g===255?null:g);
  return patch;
}
