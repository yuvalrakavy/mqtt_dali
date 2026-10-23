import test from 'node:test';
import assert from 'node:assert/strict';
import { Controller } from './controller.ts';
import { FeedbackScope,decodeFeedback,type FeedbackValues } from './feedback.ts';
import type { Snapshot,PhysicalRow } from './model.ts';

function fixture(kind:'input'|'instance'='instance'){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef',target='Bus1.Inputs.Panel.Instances.Key';
  const parent={name:'Panel',target:'Bus1.Inputs.Panel',binding_revision:1,short_address:0,instances:[{name:'Key',target,binding_revision:1,number:0,instance_type:1}]};
  ui.client.views.ready=true;ui.client.views.value={phase:'ready',identity:{controller:'Controller'},hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',revision:1,inventory_epoch:epoch,buses:[{bus:0,name:'Bus1',enabled:true,outputs:[],inputs:[parent]}],identity_ledger:{baseline:[{target:parent.target,binding_revision:1,location:{kind:'input',bus:0,short_address:0}},{target,binding_revision:1,location:{kind:'input_instance',bus:0,short_address:0,instance:0}}],batches:[]}}} as unknown as Snapshot;
  const row=ui.currentRows.find(r=>r.kind===kind) as PhysicalRow,scope=new FeedbackScope(ui,row),calls:any[]=[];
  const values:FeedbackValues={features:[32],capability:1,supported:true,active:false,quality:'complete'};
  const report=(operation='read_input_feedback')=>({operation,outcome:operation==='set_input_feedback'?'set':'current',target:row.item.target,target_generation:row.generation,revision:1,bus:0,address:0,instance:kind==='instance'?0:null,reported_type:kind==='instance'?1:null,configuration_changed:false,feedback:structuredClone(values)});
  ui.client.job=async(generation,target,command,params)=>{calls.push({generation,target,command,params:structuredClone(params)});return report(command);};
  return {ui,row,scope,calls,values,report};
}

test('feedback module and instance read/set keep original authority and actual values',async()=>{
  for(const kind of ['input','instance'] as const){
    const f=fixture(kind),saved=structuredClone(f.ui.snapshot);await f.scope.read();
    f.ui.client.job=async(generation,target,command,params:any)=>{f.calls.push({generation,target,command,params});return {...f.report(command),feedback:{...f.values,active:params.active}};};
    const actual=await f.scope.set(true);assert.equal(actual?.active,true);
    assert.deepEqual(f.calls[1],{generation:f.row.generation,target:f.row.item.target,command:'set_input_feedback',params:{expected_revision:1,active:true}});
    assert.match(f.ui.notice,/verified in hardware/);assert.deepEqual(f.ui.snapshot,saved);f.ui.client.close();
  }
});
test('feedback set refuses a reported success with mismatched actual state',async()=>{
  const f=fixture();await f.scope.read();
  await assert.rejects(f.scope.set(true),/match the requested state/,'feedback actual mismatch was accepted');
  assert.equal(f.ui.notice,'');assert.equal(f.scope.values?.active,false);f.ui.client.close();
});
test('feedback unsupported partial or unread evidence cannot submit an action',async()=>{
  for(const v of [null,{features:[],capability:null,supported:false,active:null,quality:'complete'},
    {features:null,capability:null,supported:null,active:null,quality:'partial'},
    {features:[32],capability:1,supported:true,active:null,quality:'partial'}] as (FeedbackValues|null)[]){
    const f=fixture();if(v){Object.assign(f.values,v);f.ui.client.job=async()=>{f.calls.push('read');return {...f.report(),outcome:v.quality==='partial'?'unavailable':'unsupported',feedback:v};};await f.scope.read();}
    const count=f.calls.length;let failure:unknown;try{await f.scope.set(true);}catch(e){failure=e;}
    assert.equal(f.calls.length,count,'unsupported or partial feedback submitted an action');assert.ok(failure instanceof Error);f.ui.client.close();
  }
});
test('feedback decoder rejects forged complete or inconsistent feature capability evidence',()=>{
  const base:FeedbackValues={features:[32],capability:1,supported:true,active:false,quality:'complete'};
  for(const patch of [{features:null},{features:[]},{features:[33]},{features:[32,32]},{features:[33,32]},{features:[31]},{features:[97]},
    {capability:null},{capability:0},{capability:64},{capability:256},{active:null},{active:0},{supported:false},{quality:'partial'}]){
    assert.throws(()=>decodeFeedback({...base,...patch}),/evidence/,'forged feedback evidence accepted');
  }
  const future={...base,capability:65};assert.equal(decodeFeedback(future).capability,65);
  assert.equal(decodeFeedback({...base,features:[32,33,96]}).features?.length,3);
});
test('feedback changed scopes cannot accept late reads or submit actions',async()=>{
  for(const kind of ['input','instance'] as const)for(const reason of ['cancel','epoch','root','generation','address','offline','logout']){
    const f=fixture(kind);
    f.ui.client.job=async()=>{f.calls.push('read');if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='root')f.ui.snapshot.inventory.revision++;if(reason==='generation'){const module=f.ui.snapshot.inventory.buses[0].inputs![0];if(kind==='input')module.binding_revision++;else module.instances![0].binding_revision++;}if(reason==='address')f.ui.snapshot.inventory.buses[0].inputs![0].short_address=1;if(reason==='offline')f.ui.client.views.ready=false;if(reason==='logout')f.ui.signingOut=true;return f.report();};
    await assert.rejects(f.scope.read(),undefined,'retired feedback read was accepted');assert.equal(f.scope.values,null);await assert.rejects(f.scope.set(true));assert.equal(f.calls.length,1,'retired feedback scope submitted an action');f.ui.client.close();
  }
});
test('feedback result identity operation outcome and configuration flags are verified',async()=>{
  for(const reason of ['operation','target','generation','revision','bus','address','instance','type','config','outcome']){
    const f=fixture();await f.scope.read();f.ui.client.job=async()=>{const r=f.report('set_input_feedback');r.feedback.active=true;
      if(reason==='operation')r.operation='read_input_feedback';if(reason==='target')r.target='other';if(reason==='generation')r.target_generation='other';if(reason==='revision')r.revision=2;if(reason==='bus')r.bus=1;if(reason==='address')r.address=1;if(reason==='instance')r.instance=1;if(reason==='type')r.reported_type=2;if(reason==='config')r.configuration_changed=true;if(reason==='outcome')r.outcome='unconfirmed';return r;};
    await assert.rejects(f.scope.set(true));assert.equal(f.ui.notice,'');f.ui.client.close();
  }
});
test('feedback read preserves known unsupported versus unknown evidence',async()=>{
  for(const v of [{features:[],capability:null,supported:false,active:null,quality:'complete'},
    {features:[32],capability:0,supported:false,active:null,quality:'complete'},
    {features:[32],capability:64,supported:null,active:null,quality:'partial'},
    {features:[32],capability:null,supported:null,active:null,quality:'partial'}] as FeedbackValues[]){
    const f=fixture();f.ui.client.job=async()=>({...f.report(),outcome:v.quality==='complete'?'unsupported':'unavailable',feedback:v});
    assert.deepEqual(await f.scope.read(),v);f.ui.client.close();
  }
});
test('feedback same-value activation is explicit and cancellation stops pre-submit',async()=>{
  const f=fixture();await f.scope.read();await f.scope.set(false);assert.equal(f.calls.length,2);assert.equal(f.calls[1].params.active,false);
  f.scope.cancelled=true;const count=f.calls.length;await assert.rejects(f.scope.set(true));assert.equal(f.calls.length,count,'dismissed feedback submitted an action');f.ui.client.close();
});
