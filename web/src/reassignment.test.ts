import test from 'node:test';
import assert from 'node:assert/strict';
import { Controller } from './controller.ts';
import { Reassignment, type Selection, type Space } from './reassignment.ts';
import type { Snapshot } from './model.ts';

function fixture(selection:Selection='outputs',capacity=2){
  const ui=new Controller();ui.client.views.ready=true;
  ui.client.views.value={identity:{controller:'Controller'},phase:'ready',hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',inventory_epoch:'0123456789abcdef0123456789abcdef',revision:1,buses:[{bus:0,name:'Bus1',enabled:true,outputs:[],inputs:[]}],identity_ledger:{baseline:[],batches:[]}}} as unknown as Snapshot;
  const scope=new Reassignment(ui,0,selection),calls:any[]=[],counts={outputs:0,inputs:0};
  const answer=async(generation:string,target:string|null,command:string,params:any)=>{
    calls.push({generation,target,command,params:structuredClone(params)});
    const space:Space=params.space??(command==='assign_next_output'?'outputs':'inputs');
    let result:any={bus:0,space,revision:ui.snapshot.inventory.revision};
    if(command==='reset_address_space'){result.outcome='cleared';result.revision+=2;}
    else if(command.startsWith('assign_next_')){
      assert.equal(params.space,undefined,'assignment received unsupported space parameter');
      if(counts[space]===capacity)result.outcome=`no_unaddressed_${space==='outputs'?'output':'input'}`;
      else{result.outcome='assigned';result.address=counts[space]++;result.revision+=2;}
    }else if(command==='discover_address')result={revision:result.revision,probe:{bus:0,space,quality:'complete'}};
    else throw new Error(`Unexpected command ${command}`);
    ui.client.views.value={...ui.snapshot,inventory:{...ui.snapshot.inventory,revision:result.revision}};return result;
  };
  ui.client.job=answer;return {ui,scope,calls,answer,counts};
}
test('reassignment composes selected spaces in order and advances only its own exact revisions',async()=>{
  for(const selection of ['outputs','inputs','both'] as const){
    const f=fixture(selection);await f.scope.run(()=>{});
    assert.deepEqual(f.calls.filter(c=>c.command==='reset_address_space').map(c=>c.params.space),selection==='both'?['outputs','inputs']:[selection]);
    if(selection!=='both')assert.ok(f.calls.every(c=>c.params.space===undefined||c.params.space===selection),'selected-space refresh touched the other space');
    for(const call of f.calls)assert.equal(call.generation,`0123456789abcdef0123456789abcdef.${call.params.expected_revision}`);
    // Every sent root is monotonic; the first reservation after reset uses R3,
    // not an assumed R2. Discovery's report stores scope inside probe.
    assert.equal(f.calls[1].params.expected_revision,3);
    assert.equal(f.calls[1].generation,'0123456789abcdef0123456789abcdef.3');
    for(let i=1;i<f.calls.length;i++)assert.ok(f.calls[i].params.expected_revision>=f.calls[i-1].params.expected_revision);
    if(selection==='both'){const input=f.calls.findIndex(c=>c.command==='reset_address_space'&&c.params.space==='inputs');assert.ok(f.calls.slice(0,input).filter(c=>c.command==='discover_address').length>=64);}
    f.ui.client.close();
  }
});
test('all 64 verified assignments stop at capacity without asking for a nonexistent free address',async()=>{
  const f=fixture('outputs',64);await f.scope.run(()=>{});
  assert.equal(f.calls.filter(c=>c.command==='assign_next_output').length,64);assert.equal(f.counts.outputs,64);assert.match(f.ui.notice,/64 outputs/);f.ui.client.close();
});
test('unconfirmed clear or assignment stops later operations and the other selected space',async()=>{
  for(const phase of ['clear','assign']){
    const f=fixture('both');f.ui.client.job=async(...args)=>{const result=await f.answer(...args);if(args[2]===(phase==='clear'?'reset_address_space':'assign_next_output'))result.outcome=phase==='clear'?'retired_but_unconfirmed':'reserved_but_unconfirmed';return result;};
    await assert.rejects(f.scope.run(()=>{}),/unconfirmed/);assert.equal(f.calls.length,phase==='clear'?1:2);assert.equal(f.calls.some(c=>c.command==='assign_next_input'),false);f.ui.client.close();
  }
});
test('review freezes names and roots and refuses an external edit instead of rebasing',async()=>{
  const f=fixture();f.ui.client.views.value.inventory.buses[0].name='Other';
  assert.equal(f.scope.initial.inventory.buses[0].name,'Bus1');await assert.rejects(f.scope.run(()=>{}),/changed/);assert.equal(f.calls.length,0);f.ui.client.close();
  const g=fixture();g.ui.client.job=async(...args)=>{const result=await g.answer(...args);g.ui.client.views.value.inventory.revision++;return result;};
  await assert.rejects(g.scope.run(()=>{}),/changed/);assert.equal(g.calls.length,1);g.ui.client.close();
});
test('route lifetime cancellation, reconnect and stop prevent later submissions even if global cancellation resets',async()=>{
  for(const cause of ['lifetime','reconnect','stop']){
    const f=fixture('both');f.ui.client.job=async(...args)=>{const result=await f.answer(...args);if(cause==='lifetime'){f.scope.cancelled=true;f.ui.cancelled=false;}else if(cause==='reconnect')f.ui.client.views.epoch++;else f.ui.cancelled=true;return result;};
    await assert.rejects(f.scope.run(()=>{}),/stopped/);assert.equal(f.calls.length,1);f.ui.client.close();
  }
});
test('foreign or backwards command reports are not accepted as own progress',async()=>{
  for(const fault of ['space','bus','revision']){
    const f=fixture();f.ui.client.job=async(...args)=>{const result=await f.answer(...args);result[fault]=fault==='space'?'inputs':fault==='bus'?1:0;return result;};
    await assert.rejects(f.scope.run(()=>{}),/result changed/);assert.equal(f.calls.length,1);f.ui.client.close();
  }
});
