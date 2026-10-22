import test from 'node:test';
import assert from 'node:assert/strict';
import { SettingDraft, ignored, fromStored, validSetting, type Setting } from './scene-settings.ts';
import { SceneScope, decodeSavedView, freezeChanges, applyChanges } from './scene-controller.ts';
import { Controller } from './controller.ts';
import { rows, type SceneRow } from './model.ts';

test('scene drafts preserve untouched native codes, selected MASK family and unmanaged distinction',()=>{
  const cases:Setting[]=[ignored(),{level:{kind:'value',value:127},color:{kind:'temperature',value:317}},{level:{kind:'mask'},color:{kind:'xy',x:null,y:65534}},{level:{kind:'value',value:1},color:{kind:'rgbwaf',channels:[1,80,null,254,null,0],control:128}},{level:{kind:'mask'},color:{kind:'temperature',value:null}}];
  for(const saved of cases){const d=new SettingDraft(saved);assert.deepEqual(d.setting(),saved,'display roundtrip changed native scene values');d.change('brightness','50');if(d.action!=='ignore')assert.deepEqual(d.setting().color,saved.color,'brightness edit changed untouched colour');}
  const unmanaged=new SettingDraft(null);assert.equal(unmanaged.original,null);assert.equal(unmanaged.dirty,false);
  const d=new SettingDraft(cases[1]);d.chooseAction('keep');assert.equal(d.setting().level.kind,'mask');assert.deepEqual(d.setting().color,cases[1].color);d.chooseAction('ignore');assert.deepEqual(d.setting(),ignored());
  d.chooseAction('brightness');d.change('brightness','101');assert.throws(()=>d.setting(),/brightness/);d.change('brightness','');assert.throws(()=>d.setting(),/required/);
  const mask=new SettingDraft(ignored());mask.chooseFamily('temperature');assert.deepEqual(mask.setting(),{level:{kind:'mask'},color:{kind:'temperature',value:null}});mask.change('temperature','4000');assert.deepEqual(mask.setting().color,{kind:'temperature',value:250});
  assert.equal(validSetting({level:{kind:'mask'},color:{kind:'temperature'}}),false,'missing attribute became MASK');
});
test('scene row check and blur share a save and failed drafts retain text',async()=>{
  const d=new SettingDraft(ignored());d.chooseAction('off');let calls=0,release!:()=>void;const held=new Promise<void>(r=>release=r);
  const one=d.commit(async()=>{calls++;await held;}),two=d.commit(async()=>{calls++;});release();await Promise.all([one,two]);assert.equal(calls,1,'scene blur/check duplicated a physical save');
  d.change('brightness','wrong');d.chooseAction('brightness');await assert.rejects(d.commit(async()=>{calls++;}));assert.equal(d.fields.brightness,'wrong');assert.equal(d.dirty,true);assert.equal(calls,1);
});
function fixture(){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef';
  const outputs=[{name:'A',target:'Bus.Outputs.A',short_address:0,binding_revision:1},{name:'B',target:'Bus.Outputs.B',short_address:1,binding_revision:1}],scene={name:'Evening',target:'Bus.Scenes.Evening',slot:3,binding_revision:1};
  ui.client.views.ready=true;ui.client.views.value={phase:'ready',hardware:{state:'ready',buses:[{bus:0,status:2}]},inventory:{inventory_epoch:epoch,revision:1,name:'Controller',buses:[{bus:0,name:'Bus',enabled:true,outputs,scenes:[scene]}],identity_ledger:{baseline:[...outputs.map(o=>({target:o.target,binding_revision:1,location:{kind:'output',bus:0,short_address:o.short_address}})),{target:scene.target,binding_revision:1,location:{kind:'scene',bus:0,slot:3}}],batches:[]}},states:[]};
  return {ui,epoch,scene:rows(ui.snapshot).find((r):r is SceneRow=>r.kind==='scene')!};
}
test('saved scene replies require every current fixture and original scene/root tokens',()=>{
  const {ui,epoch,scene}=fixture(),scope=new SceneScope(ui,0,3);
  const value={target:scene.item.target,target_generation:scene.generation,revision:1,bus:0,slot:3,fixtures:scope.outputs.map(row=>({fixture:row.item.target,fixture_generation:`${epoch}.1`,saved:null}))};
  assert.equal(decodeSavedView(value,ui.snapshot,scene).size,2);
  for(const bad of [{...value,fixtures:value.fixtures.slice(1)},{...value,fixtures:[value.fixtures[0],value.fixtures[0]]},{...value,revision:2},{...value,target_generation:`${epoch}.2`}])assert.throws(()=>decodeSavedView(bad,ui.snapshot,scene),/changed|incomplete/,'partial/stale saved scene became unmanaged data');ui.client.close();
});
test('scene batch freezes reviewed values and advances only its own exact revisions',async()=>{
  const {ui,scene}=fixture(),scope=new SceneScope(ui,0,3),setting=ignored();
  const changes=freezeChanges(scope.outputs.map(row=>({row,before:null,after:setting,command:'set_scene_row' as const})));setting.level={kind:'value',value:42};
  const expected:number[]=[];ui.client.job=async(generation,target,command,params:any)=>{
    expected.push(params.expected_revision);assert.deepEqual(params.setting,ignored(),'copy/capture candidate changed after review');assert.equal(generation,scene.generation);
    const revision=ui.snapshot.inventory.revision+1;ui.client.views.value={...ui.snapshot,inventory:{...ui.snapshot.inventory,revision}};
    return {target,target_generation:generation,fixture:params.fixture,fixture_generation:scene.generation,bus:0,slot:3,revision,outcome:'verified'};
  };
  await applyChanges(scope,changes,()=>{});assert.deepEqual(expected,[1,2],'scene batch reused/rebased expected revision');
  ui.snapshot.inventory.revision=4;await assert.rejects(scope.mutate(scope.outputs[0],'set_scene_row',{setting:ignored()}),/changed/,'external edit was silently rebased');assert.equal(expected.length,2,'external edit was silently rebased');ui.client.close();
});
test('scene completion cannot authorize another row after reconnect or page cancellation',async()=>{
  for(const mode of ['epoch','cancel','external']){
    const {ui,scene}=fixture(),scope=new SceneScope(ui,0,3);let calls=0;
    ui.client.job=async(generation,target,_command,params:any)=>{calls++;const revision=2;ui.client.views.value={...ui.snapshot,inventory:{...ui.snapshot.inventory,revision:mode==='external'?3:2}};if(mode==='epoch')ui.client.views.begin();if(mode==='cancel')scope.cancelled=true;return {target,target_generation:generation,fixture:params.fixture,fixture_generation:scene.generation,bus:0,slot:3,revision,outcome:'verified'};};
    await assert.rejects(applyChanges(scope,freezeChanges(scope.outputs.map(row=>({row,before:null,after:ignored(),command:'set_scene_row'}))),()=>{}),/changed|stopped/);assert.equal(calls,1,'cancelled or externally changed scene continued its batch');ui.client.close();
  }
});
test('retired scene scope refuses submission after a completed reconnect or cancellation',async()=>{
  for(const mode of ['epoch','cancel']){
    const {ui,scene}=fixture(),scope=new SceneScope(ui,0,3);let calls=0;
    if(mode==='epoch'){ui.client.views.begin();ui.client.views.ready=true;}else scope.cancelled=true;
    ui.client.job=async(generation,target,_command,params:any)=>{calls++;return {target,target_generation:generation,fixture:params.fixture,fixture_generation:scene.generation,bus:0,slot:3,revision:1,outcome:'verified'};};
    await assert.rejects(scope.mutate(scope.outputs[0],'set_scene_row',{setting:ignored()}),/stopped/);
    assert.equal(calls,0,'retired scene scope submitted work');ui.client.close();
  }
});
test('scene adoption candidates preserve unknowns and reject incomplete or unsupported evidence',()=>{
  const stored={quality:'complete',stale:false,dt8:true,level:{kind:'mask',raw:255},color_type:{kind:'value',raw:0x10},values:{x:{kind:'mask',raw:65535},y:{kind:'value',raw:20000}}};
  assert.deepEqual(fromStored(stored),{level:{kind:'mask'},color:{kind:'xy',x:null,y:20000}});
  assert.equal(fromStored({...stored,quality:'partial'}),null,'partial native scene became an adoption candidate');assert.equal(fromStored({...stored,stale:true}),null,'stale native scene became an adoption candidate');assert.equal(fromStored({...stored,values:{...stored.values,extra:{kind:'value',raw:0}}}),null);assert.equal(fromStored({...stored,color_type:{kind:'value',raw:0x40}}),null);
});

test('scene unconfirmed save stops later writes and reviewed repair carries exact evidence',async()=>{
  const {ui,scene}=fixture(),scope=new SceneScope(ui,0,3);let calls=0;
  const reviewed={quality:'complete',stale:false,level:{kind:'value',raw:42},dt8:false,color_type:null,values:{},reason:null};
  ui.client.job=async(generation,target,command,params:any)=>{calls++;assert.equal(command,'repair_scene_row');assert.deepEqual(params.reviewed,reviewed,'reviewed native evidence changed at submission');assert.equal(params.expected_revision,calls);ui.client.views.value={...ui.snapshot,inventory:{...ui.snapshot.inventory,revision:2}};return {target,target_generation:generation,fixture:params.fixture,fixture_generation:scene.generation,bus:0,slot:3,revision:2,outcome:'unconfirmed'};};
  await assert.rejects(applyChanges(scope,freezeChanges(scope.outputs.map(row=>({row,before:ignored(),after:ignored(),reviewed,command:'repair_scene_row'}))),()=>{}),/Saved settings are not confirmed/,'unconfirmed scene continued programming later rows');
  assert.equal(calls,1,'unconfirmed scene continued programming later rows');assert.equal(scope.revision,2,'durable scene revision was lost after unconfirmed programming');assert.equal(ui.snapshot.inventory.revision,2);ui.client.close();
});

test('scene batch accepts its own skipped reserved revision and then pins that exact revision',async()=>{
  const {ui,scene}=fixture(),scope=new SceneScope(ui,0,3),expected:number[]=[];
  ui.client.job=async(generation,target,_command,params:any)=>{
    expected.push(params.expected_revision);const revision=expected.length===1?3:4;
    ui.client.views.value={...ui.snapshot,inventory:{...ui.snapshot.inventory,revision}};
    return {target,target_generation:generation,fixture:params.fixture,fixture_generation:scene.generation,bus:0,slot:3,revision,outcome:'verified'};
  };
  await assert.doesNotReject(applyChanges(scope,freezeChanges(scope.outputs.map(row=>({row,before:null,after:ignored(),command:'set_scene_row'}))),()=>{}),'valid consumed revision rejected after recovery');
  assert.deepEqual(expected,[1,3],'scene batch failed to pin its own skipped revision');assert.equal(scope.revision,4);ui.client.close();
});
