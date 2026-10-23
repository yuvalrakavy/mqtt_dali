import test from 'node:test';
import assert from 'node:assert/strict';
import { Controller } from './controller.ts';
import { InputSettingsScope, changedSettings, type InputValues } from './input-settings.ts';
import type { Snapshot,PhysicalRow } from './model.ts';
import {timerRaw,timerDisplay,decodeButtonTimers} from './button-timing.ts';

function fixture(){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef',target='Bus1.Inputs.Panel.Instances.Key';
  const parent={name:'Panel',target:'Bus1.Inputs.Panel',binding_revision:1,short_address:0,instances:[{name:'Key',target,binding_revision:1,number:0,instance_type:1}]};
  ui.client.views.ready=true;
  ui.client.views.value={phase:'ready',identity:{controller:'Controller'},hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',revision:1,inventory_epoch:epoch,buses:[{bus:0,name:'Bus1',enabled:true,outputs:[],inputs:[parent]}],identity_ledger:{baseline:[{target:parent.target,binding_revision:1,location:{kind:'input',bus:0,short_address:0}},{target,binding_revision:1,location:{kind:'input_instance',bus:0,short_address:0,instance:0}}],batches:[]}}} as unknown as Snapshot;
  const row=ui.currentRows.find(r=>r.kind==='instance') as PhysicalRow,scope=new InputSettingsScope(ui,row),calls:any[]=[];
  const values:InputValues={enabled:true,event_priority:4,event_scheme:2,event_filter:255,filter_width:8,filter_bytes:[255,null,null],groups:[255,255,255],quality:'complete'};
  const report=(operation='read_input_settings')=>({operation,outcome:operation==='configure_input'?'configured':'current',target,target_generation:row.generation,revision:1,bus:0,address:0,instance:0,reported_type:1,configuration_changed:false,settings:structuredClone(values)});
  ui.client.job=async(generation,target,command,params)=>{calls.push({generation,target,command,params:structuredClone(params)});return report(command);};
  return {ui,row,scope,calls,values,report};
}

test('input settings read/save uses frozen authority and only changed fields',async()=>{
  const f=fixture();const before=await f.scope.read(),after={...before,enabled:false,groups:[0,255,31]};
  const patch=changedSettings(before,after);assert.deepEqual(patch,{enabled:false,groups:[0,null,31]});
  f.ui.client.job=async(generation,target,command,params)=>{f.calls.push({generation,target,command,params});return {...f.report(command),settings:after};};
  await f.scope.save(patch);assert.equal(f.calls[1].generation,f.row.generation);assert.equal(f.calls[1].params.expected_revision,1);assert.deepEqual(f.calls[1].params.settings,patch);assert.match(f.ui.notice,/verified in hardware/);f.ui.client.close();
});
test('input settings changed scopes cannot submit and late reads cannot authorize a write',async()=>{
  for(const reason of ['cancel','epoch','root','generation','offline']){
    const f=fixture();f.ui.client.job=async()=>{f.calls.push('read');if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='root')f.ui.snapshot.inventory.revision++;if(reason==='generation')f.ui.snapshot.inventory.buses[0].inputs![0].instances![0].binding_revision++;if(reason==='offline')f.ui.client.views.ready=false;return f.report();};
    await assert.rejects(f.scope.read());assert.equal(f.scope.values,null);await assert.rejects(f.scope.save({enabled:false}));assert.equal(f.calls.length,1);f.ui.client.close();
  }
});
test('input settings reject partial, forged, unconfirmed or mismatched evidence',async()=>{
  for(const reason of ['partial','filter','groups','width','generation','revision','outcome','match']){
    const f=fixture();await f.scope.read();f.ui.client.job=async()=>{const r=f.report('configure_input');if(reason!=='match')r.settings.enabled=false;if(reason==='partial')r.settings.quality='partial';if(reason==='filter')r.settings.filter_bytes=[254,null,null];if(reason==='groups')r.settings.groups=[255];if(reason==='width')r.settings.filter_width=24;if(reason==='generation')r.target_generation='other';if(reason==='revision')r.revision=2;if(reason==='outcome')r.outcome='unconfirmed';return r;};
    await assert.rejects(f.scope.save({enabled:false}));assert.equal(f.ui.notice,'');f.ui.client.close();
  }
});
test('input settings frozen root is checked again before save and unchanged drafts do not write',async()=>{
  const f=fixture();const v=await f.scope.read();await assert.rejects(f.scope.save(changedSettings(v,v)),/Change at least/);f.ui.snapshot.inventory.revision++;await assert.rejects(f.scope.save({enabled:false}),/changed/);assert.equal(f.calls.length,1);f.ui.client.close();
});

test('button timing save requires actual matching timer evidence',async()=>{
  const f=fixture();Object.assign(f.values,{button_timers:{short:25,short_min:25,double:0,double_min:10,repeat:8,stuck:20,quality:'complete'}});
  await f.scope.read();
  await assert.rejects(f.scope.save({button_timers:{short:50}} as any),/timing.*match/i);
  assert.equal(f.ui.notice,'');f.ui.client.close();
});
test('button timing invalid device minimum is refused before submitting',async()=>{
  const f=fixture();Object.assign(f.values,{button_timers:{short:25,short_min:25,double:0,double_min:10,repeat:8,stuck:20,quality:'complete'}});
  await f.scope.read();const calls=f.calls.length;
  let failure:unknown;try{await f.scope.save({enabled:false,button_timers:{short:24}});}catch(e){failure=e;}
  assert.equal(f.calls.length,calls,'invalid timing sent a mixed hardware patch');
  assert.ok(failure instanceof Error);assert.match(failure.message,/minimum|range/i);f.ui.client.close();
});
test('button timing exact units and changed-only patch preserve native representations',async()=>{
  assert.equal(timerRaw('short','1000'),50,'timer UI conversion changed native units');assert.equal(timerRaw('double','0'),0);
  assert.equal(timerRaw('repeat','160'),8);assert.equal(timerRaw('stuck','255'),255);
  assert.equal(timerDisplay('short',255),5100);assert.equal(timerDisplay('stuck',255),255);
  const f=fixture();const timers={short:25,short_min:25,double:0,double_min:10,repeat:8,stuck:20,quality:'complete' as const};
  Object.assign(f.values,{button_timers:timers});const before=await f.scope.read();
  const after={...before,button_timers:{...timers,short:timerRaw('short','1000')}};
  assert.deepEqual(changedSettings(before,after),{button_timers:{short:50}});
  f.ui.client.job=async(_g,_t,command)=>({...f.report(command),settings:after});await f.scope.save({button_timers:{short:50}});
  f.ui.client.close();
});
test('button timing validates claimed complete evidence and native steps',async()=>{
  const timers={short:25,short_min:25,double:0,double_min:10,repeat:8,stuck:20,quality:'complete' as const};
  for(const patch of [{short_min:null},{double_min:255},{double_min:0},{short:24},{double:101},{repeat:4},{stuck:4}]){
    assert.throws(()=>decodeButtonTimers({...timers,...patch}),/evidence/);
  }
  const f=fixture();Object.assign(f.values,{button_timers:timers});await f.scope.read();const calls=f.calls.length;
  await assert.rejects(f.scope.save({button_timers:{short:timerRaw('short','1001')}}),/native steps/);
  assert.equal(f.calls.length,calls);f.ui.client.close();
});
test('partial timer evidence allows common-only settings but refuses timer changes',async()=>{
  const f=fixture();const timers={short:25,short_min:25,double:0,double_min:null,repeat:8,stuck:20,quality:'partial' as const};
  Object.assign(f.values,{button_timers:timers});await f.scope.read();
  await assert.rejects(f.scope.save({button_timers:{double:0}}),/Complete button timing/);
  f.ui.client.job=async(_g,_t,command)=>({...f.report(command),settings:{...f.values,enabled:false}});
  await f.scope.save({enabled:false});assert.match(f.ui.notice,/verified/);f.ui.client.close();
});
