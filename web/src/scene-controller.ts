import type { Controller } from './controller.ts';
import { rows, busReady, type SceneRow, type PhysicalRow, type Snapshot } from './model.ts';
import { validSetting, type Setting } from './scene-settings.ts';

export interface SavedView {target:string;target_generation:string;revision:number;bus:number;slot:number;fixtures:{fixture:string;fixture_generation:string;saved:Setting|null}[]}
export function decodeSavedView(value:any,snapshot:Snapshot,scene:SceneRow):Map<string,Setting|null> {
  if(!value||value.target!==scene.item.target||value.target_generation!==scene.generation||value.revision!==snapshot.inventory.revision||value.bus!==scene.bus.bus||value.slot!==scene.item.slot||!Array.isArray(value.fixtures))throw new Error('Saved scene view changed. Refresh it.');
  const outputs=rows(snapshot).filter(r=>r.kind==='output'&&r.bus.bus===scene.bus.bus),found=new Map<string,Setting|null>();
  for(const entry of value.fixtures){const row=outputs.find(r=>r.item.target===entry.fixture);if(!row||row.generation!==entry.fixture_generation||found.has(row.key)||entry.saved!==null&&!validSetting(entry.saved))throw new Error('Saved scene view is incomplete or changed.');found.set(row.key,structuredClone(entry.saved));}
  if(found.size!==outputs.length)throw new Error('Saved scene view is incomplete.');return found;
}
export class SceneScope {
  readonly initial:Snapshot;readonly epoch:number;readonly scene?:SceneRow;
  readonly outputs:PhysicalRow[];readonly bus:number;readonly slot:number;
  revision:number;cancelled=false;
  readonly ui:Controller;
  constructor(ui:Controller,bus:number,slot:number){
    this.ui=ui;
    this.initial=structuredClone(ui.snapshot);this.epoch=ui.client.views.epoch;this.bus=bus;this.slot=slot;this.revision=this.initial.inventory.revision;
    const all=rows(this.initial);this.scene=all.find((r):r is SceneRow=>r.kind==='scene'&&r.bus.bus===bus&&r.item.slot===slot);
    this.outputs=all.filter((r):r is PhysicalRow=>r.kind==='output'&&r.bus.bus===bus);
  }
  assert(hardware=true){
    const s=this.ui.snapshot,before=this.initial.inventory,after=s.inventory;
    const bus=after.buses.find(b=>b.bus===this.bus),old=before.buses.find(b=>b.bus===this.bus);
    if(this.cancelled||hardware&&this.ui.cancelled||this.ui.signingOut||this.epoch!==this.ui.client.views.epoch||!this.ui.client.views.ready)throw new Error('Operation stopped. Review before continuing.');
    if(before.inventory_epoch!==after.inventory_epoch||after.revision!==this.revision||!bus||!old||bus.name!==old.name||s.phase!=='ready'||hardware&&!busReady(s,bus))throw new Error('Configuration or connection changed. Review the scene again.');
    if(this.scene){const current=rows(s).find(r=>r.key===this.scene!.key);if(!current||current.generation!==this.scene.generation||current.item.target!==this.scene.item.target)throw new Error('Selected scene changed. Review it again.');}
  }
  fixture(row:PhysicalRow){this.assert();const current=this.ui.currentRows.find(r=>r.key===row.key);if(!current||current.generation!==row.generation||current.item.target!==row.item.target)throw new Error('Selected fixture changed. Review it again.');}
  async saved():Promise<Map<string,Setting|null>>{
    this.assert(false);if(!this.scene)return new Map(this.outputs.map(r=>[r.key,null]));
    const result=await this.ui.client.job(this.scene.generation,this.scene.item.target,'read_scene_settings');this.assert(false);return decodeSavedView(result,this.initial,this.scene);
  }
  async inspect(row:PhysicalRow,slot=this.slot,capture=false){
    this.fixture(row);const result=await this.ui.client.job(row.generation,row.item.target,capture?'capture_scene_row':'read_scene',{slot});this.fixture(row);
    const target=capture?result.fixture:result.target,generation=capture?result.fixture_generation:result.target_generation;
    if(target!==row.item.target||generation!==row.generation||result.revision!==this.revision||result.bus!==this.bus||result.slot!==slot)throw new Error('Fixture evidence changed. Inspect again.');return structuredClone(result);
  }
  async mutate(row:PhysicalRow,command:'set_scene_row'|'adopt_scene_row'|'repair_scene_row',data:{setting?:Setting;reviewed?:unknown}){
    this.fixture(row);const scene=this.scene;if(!scene)throw new Error('Name this scene before saving settings.');
    const result=await this.ui.client.job(scene.generation,scene.item.target,command,{...structuredClone(data),fixture:row.item.target,expected_revision:this.revision});
    // The completion may arrive after a slot change or reconnect. Never use it
    // as permission for a subsequent row or merge it into public stream state.
    if(!Number.isSafeInteger(result.revision)||result.revision<this.revision||result.target!==scene.item.target||result.target_generation!==scene.generation||result.fixture!==row.item.target||result.fixture_generation!==row.generation||result.bus!==this.bus||result.slot!==this.slot)throw new Error('Scene result changed. Inspect configuration before retrying.');
    if(this.cancelled||this.ui.cancelled||this.ui.signingOut||this.epoch!==this.ui.client.views.epoch||!this.ui.client.views.ready)throw new Error('Operation stopped. Inspect saved settings before retrying.');
    await this.ui.changed(result.revision);this.revision=result.revision;this.fixture(row);
    if(result.outcome!=='verified'&&result.outcome!=='adopted')throw new Error('Saved settings are not confirmed in hardware. Inspect and repair explicitly.');return result;
  }
  async recall(group?:number){
    this.assert();const scene=this.scene;if(!scene)throw new Error('Name this scene before recall.');
    const result=await this.ui.client.job(scene.generation,scene.item.target,'recall_scene',{expected_revision:this.revision,...(group===undefined?{}:{group})});this.assert();
    if(result.outcome!=='sent')throw new Error('Recall was not confirmed as sent. Inspect the lights.');return result;
  }
}
export interface Change {row:PhysicalRow;before:Setting|null;after:Setting;reviewed?:unknown;command:'set_scene_row'|'adopt_scene_row'|'repair_scene_row'}
export function freezeChanges(changes:Change[]):Change[]{return structuredClone(changes);}
export async function applyChanges(scope:SceneScope,changes:Change[],progress:(text:string)=>void){
  for(const change of changes){scope.fixture(change.row);progress(`Saving scene ${scope.slot} · ${change.row.item.name}`);await scope.mutate(change.row,change.command,change.command==='set_scene_row'?{setting:change.after}:{reviewed:change.reviewed});}
}
