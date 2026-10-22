import test from 'node:test';
import assert from 'node:assert/strict';
import { Controller } from './controller.ts';
import { Restoration } from './restoration.ts';
import type { PhysicalRow } from './model.ts';

function fixture(input=false){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef',kind=input?'input':'output',space=input?'Inputs':'Outputs';
  const items=[{name:'Original',target:`Bus.${space}.Original`,binding_revision:1,short_address:0,instances:[]},{name:'Temporary',target:`Bus.${space}.Temporary`,binding_revision:2,short_address:1,instances:[]}];
  ui.client.views.ready=true;ui.client.views.value={identity:{controller:'Controller'},phase:'ready',hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',inventory_epoch:epoch,revision:3,buses:[{bus:0,name:'Bus',enabled:true,outputs:input?[]:items,inputs:input?items:[]}],identity_ledger:{baseline:items.map(item=>({target:item.target,binding_revision:item.binding_revision,location:{kind,bus:0,short_address:item.short_address}})),batches:[]}}};
  const old=ui.currentRows[0] as PhysicalRow,candidate=ui.currentRows[1] as PhysicalRow,scope=new Restoration(ui,old),calls:any[]=[];
  scope.candidates.add(candidate.key);
  const answer=async(generation:string,target:string|null,command:string,params:any)=>{
    calls.push({generation,target,command,params:structuredClone(params)});
    if(command==='stop_blink'){ui.client.views.value.identification=null;return {phase:'restored'};}
    if(command.startsWith('restore_')){
      const identified=command==='restore_identified_address',revision=identified?5:3;
      if(identified){const inventory=structuredClone(ui.snapshot.inventory),bus=inventory.buses[0],list=input?bus.inputs!:bus.outputs;list.splice(1,1);list[0].short_address=1;list[0].binding_revision=revision;const from=inventory.identity_ledger.baseline[0],to={...from,binding_revision:revision,location:{...from.location,short_address:1}};inventory.identity_ledger.batches.push({revision,transitions:[{from:inventory.identity_ledger.baseline[1],to:null},{from,to}]});inventory.revision=revision;ui.client.views.value={...ui.snapshot,inventory};}
      return {operation:command,outcome:'restored',target:old.item.target,target_generation:old.generation,current_generation:identified?`${epoch}.${revision}`:old.generation,bus:0,previous_address:0,address:identified?1:0,revision,candidate:identified?candidate.item.target:null,candidate_generation:identified?candidate.generation:null,source_address:identified?1:null,configuration_changed:identified};
    }
    if(command==='discover_address')return {revision:ui.snapshot.inventory.revision,probe:{bus:0,space:input?'inputs':'outputs'}};
    if(command==='read_output'||command==='read_input')return {values:{}};
    throw new Error(`Unexpected command ${command}`);
  };
  ui.client.job=answer;return {ui,scope,old,candidate,calls,answer,epoch};
}
test('unique restoration accepts unchanged revision and preserves original authority during refresh',async()=>{
  for(const input of [false,true]){const f=fixture(input);await f.scope.review().run();assert.equal(f.calls[0].generation,`${f.epoch}.1`);assert.equal(f.calls[0].params.expected_revision,3);assert.equal(f.scope.revision,3);assert.match(f.ui.notice,/Address restored/);assert.equal(f.calls[1].params.space,input?'inputs':'outputs');assert.equal(f.calls[1].params.address,0);f.ui.client.close();}
});
test('recovery can select a fixture already assigned by the normal bus scan',async()=>{
  for(const input of [false,true]){
    const f=fixture(input),scope=new Restoration(f.ui,f.old);
    await scope.review(f.candidate).run();
    assert.equal(f.calls[0].command,'restore_identified_address');
    assert.equal(f.ui.currentRows[0].key,f.old.key);
    assert.equal(f.ui.currentRows[0].item.short_address,1);
    f.ui.client.close();
  }
});
test('identified restoration waits for confirmed stop and retires only the candidate using exact own roots',async()=>{
  const f=fixture();f.ui.client.views.value.identification={target:f.candidate.item.target,generation:f.candidate.generation,phase:'blinking',actual_level:42};
  await f.scope.review(f.candidate).run();assert.deepEqual(f.calls.slice(0,2).map(c=>c.command),['stop_blink','restore_identified_address']);
  assert.equal(f.calls[1].generation,`${f.epoch}.1`);assert.equal(f.calls[1].params.candidate_generation,`${f.epoch}.2`);assert.equal(f.calls[1].params.expected_revision,3);assert.equal(f.calls[2].params.expected_revision,5);assert.equal(f.calls[2].params.address,1,'refresh did not follow the adopted address');assert.equal(f.calls[3].generation,`${f.epoch}.5`,'refresh reused the previous mapping token');assert.equal(f.scope.revision,5);assert.equal(f.ui.currentRows[0].key,f.old.key);assert.equal(f.ui.currentRows[0].item.short_address,1);f.ui.client.close();
});
test('unconfirmed stop and other blink claims prevent any address move',async()=>{
  for(const other of [false,true]){const f=fixture();f.ui.client.views.value.identification={target:other?f.old.item.target:f.candidate.item.target,generation:other?f.old.generation:f.candidate.generation,phase:'blinking',actual_level:42};f.ui.client.job=async(...args)=>{const result=await f.answer(...args);if(args[2]==='stop_blink')result.phase='interrupted';return result;};await assert.rejects(f.scope.review(f.candidate).run(),/blink|Blink/);assert.equal(f.calls.some(c=>c.command.startsWith('restore_')),false);f.ui.client.close();}
});
test('frozen restoration review refuses changed root or candidate and never refreshes its authority',async()=>{
  for(const changed of ['root','candidate']){const f=fixture(),review=f.scope.review(f.candidate);if(changed==='root')f.ui.client.views.value.inventory.revision++;else f.ui.client.views.value.inventory.buses[0].outputs[1].binding_revision++;await assert.rejects(review.run(),/changed/);assert.equal(f.calls.length,0);f.ui.client.close();}
});
test('route lifetime, reconnect, external root and cancelled scope prevent followup refresh after a reply',async()=>{
  for(const cause of ['lifetime','reconnect','root','stop']){const f=fixture();f.ui.client.job=async(...args)=>{const result=await f.answer(...args);if(cause==='lifetime'){f.scope.cancelled=true;f.ui.cancelled=false;}else if(cause==='reconnect')f.ui.client.views.epoch++;else if(cause==='root')f.ui.client.views.value.inventory.revision++;else f.ui.cancelled=true;return result;};await assert.rejects(f.scope.review().run(),/stopped|changed/);assert.equal(f.calls.length,1,'recovery continued after its view lifetime ended');f.ui.client.close();}
});
test('partial candidate retirement keeps its own committed root and successful restore stays successful when refresh fails',async()=>{
  const f=fixture();f.ui.client.job=async(...args)=>{const result=await f.answer(...args);if(args[2]==='restore_identified_address')result.outcome='configuration_updated_but_unconfirmed';return result;};await assert.rejects(f.scope.review(f.candidate).run(),/Saved address updated.*unconfirmed/);assert.equal(f.scope.revision,5);assert.equal(f.scope.address,1);assert.equal(f.calls.length,1);f.ui.client.close();
  const g=fixture();g.ui.client.job=async(...args)=>{if(args[2]==='discover_address')throw new Error('refresh failed');return g.answer(...args);};await g.scope.review().run();assert.match(g.ui.notice,/Address restored.*refresh did not finish/);g.ui.client.close();
});
test('none, multiple and foreign reports stop before followup discovery',async()=>{
  for(const outcome of ['no_unaddressed_fixture','multiple_unaddressed_fixtures','unconfirmed','foreign']){const f=fixture();f.ui.client.job=async(...args)=>{const result=await f.answer(...args);if(outcome==='foreign')result.target='Other';else result.outcome=outcome;return result;};await assert.rejects(f.scope.review().run(),/No unaddressed|Multiple|unconfirmed|result changed/);assert.equal(f.calls.length,1);f.ui.client.close();}
});
