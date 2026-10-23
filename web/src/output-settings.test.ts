import test from 'node:test';
import assert from 'node:assert/strict';
import {levelPercent,percentLevel,decodeOutputValues,OutputSettingsScope,changedOutputSettings,validateOutputPatch,outputWritable,type OutputValues} from './output-settings.ts';
import {Controller} from './controller.ts';
import type {Snapshot,PhysicalRow} from './model.ts';

const values:OutputValues={physical_minimum:1,minimum_level:1,maximum_level:254,power_on_level:255,system_failure_level:255,fade_raw:0xff,fade_time:15,fade_rate:15,quality:'complete'};
test('output settings percentages preserve every ordinary native code and refuse empty/nonfinite input',()=>{
  for(let raw=0;raw<=254;raw++)assert.equal(percentLevel(levelPercent(raw)),raw,'settings percentage changed native level code');
  assert.equal(levelPercent(254),'100.0');assert.equal(percentLevel('50'),127);
  for(const text of ['',' ','NaN','Infinity','-1','100.1'])assert.ok(Number.isNaN(percentLevel(text)),'invalid percentage became a setting');
});
test('output settings packed FF is numeric and reserved rate0 remains repairable evidence',()=>{
  assert.deepEqual(decodeOutputValues(values),values);
  const reserved={...values,fade_raw:0xf0,fade_rate:null,quality:'partial' as const};assert.deepEqual(decodeOutputValues(reserved),reserved);
  for(const patch of [{fade_time:1},{fade_rate:null},{fade_raw:null},{quality:'partial'},{minimum_level:0},{power_on_level:256},{maximum_level:NaN}])assert.throws(()=>decodeOutputValues({...values,...patch}),/evidence/,'inconsistent settings byte/quality accepted');
});

function fixture(){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef',target='Bus1.Outputs.Lamp';
  const output={name:'Lamp',target,binding_revision:1,short_address:0};ui.client.views.ready=true;
  ui.client.views.value={phase:'ready',identity:{controller:'Controller'},hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',revision:1,inventory_epoch:epoch,buses:[{bus:0,name:'Bus1',enabled:true,outputs:[output]}],identity_ledger:{baseline:[{target,binding_revision:1,location:{kind:'output',bus:0,short_address:0}}],batches:[]}}} as unknown as Snapshot;
  const row=ui.currentRows[0] as PhysicalRow,scope=new OutputSettingsScope(ui,row),calls:any[]=[];
  const report=(command='read_output_settings',v:OutputValues=values)=>{
    const readings:any={};for(const key of ['physical_minimum','minimum_level','maximum_level','power_on_level','system_failure_level'] as const)readings[key]=v[key]===null?{kind:'no_response'}:{kind:'value',raw:v[key]};
    Object.assign(readings,{fade_time_rate:v.fade_raw===null?{kind:'no_response'}:{kind:'value',raw:v.fade_raw},status:{kind:'value',raw:4},closing_status:{kind:'value',raw:4},actual_level:{kind:'value',raw:100},version:{kind:'value',raw:8}});
    return {operation:command,outcome:command==='configure_output'?'configured':v.quality==='complete'?'current':'partial',target,target_generation:row.generation,revision:1,bus:0,address:0,configuration_changed:false,settings:structuredClone(v),readings};
  };
  ui.client.job=async(g,t,c,p)=>{calls.push({generation:g,target:t,command:c,params:structuredClone(p)});return report(c);};
  return {ui,row,scope,calls,report};
}
test('output settings read and changed-only save use original authority and actual reply',async()=>{
  const f=fixture();await f.scope.read();const after={...values,minimum_level:5,power_on_level:0,system_failure_level:255,fade_raw:0x1f,fade_time:1};
  const patch=changedOutputSettings(values,after);assert.deepEqual(patch,{minimum_level:5,power_on_level:0,fade_time:1});
  f.ui.client.job=async(g,t,c,p)=>{f.calls.push({generation:g,target:t,command:c,params:p});return f.report(c,after);};
  await f.scope.save(patch);assert.equal(f.calls[1].generation,f.row.generation);assert.deepEqual(f.calls[1].params,{expected_revision:1,settings:patch});assert.match(f.ui.notice,/verified in hardware/);f.ui.client.close();
});
test('output settings reject entire invalid mixed patch before submitting any field',async()=>{
  const f=fixture();await f.scope.read();
  for(const patch of [{minimum_level:5,fade_rate:0},{minimum_level:0},{minimum_level:200,maximum_level:100},{power_on_level:null},{minimum_level:2.5},{fade_time:16},{physical_minimum:1},{}])await assert.rejects(f.scope.save(patch as any));
  assert.equal(f.calls.length,1,'invalid mixed settings patch reached hardware');
  assert.throws(()=>validateOutputPatch({minimum_level:2},{...values,physical_minimum:5,minimum_level:5}),/physical minimum/);f.ui.client.close();
});
test('output settings partial unrequested families and readable reserved rate remain usable',async()=>{
  const f=fixture(),reserved={...values,maximum_level:null,fade_raw:0xf0,fade_rate:null,quality:'partial' as const};
  f.ui.client.job=async(_g,_t,c)=>f.report(c,reserved);await f.scope.read();assert.equal(outputWritable('minimum_level',f.scope.values),false);assert.equal(outputWritable('fade_rate',f.scope.values),true,'readable reserved fade rate could not be repaired');
  const after={...reserved,fade_raw:0xf1,fade_rate:1,power_on_level:0};f.ui.client.job=async(_g,_t,c)=>f.report(c,after);
  await f.scope.save({fade_rate:1,power_on_level:0});assert.equal(f.scope.values!.quality,'partial');assert.equal(f.scope.values!.maximum_level,null);f.ui.client.close();
});
test('output settings late reads retire original scope without authorizing a write',async()=>{
  for(const reason of ['cancel','epoch','root','generation','offline','address','disabled']){
    const f=fixture();f.ui.client.job=async()=>{f.calls.push('read');if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='root')f.ui.snapshot.inventory.revision++;if(reason==='generation')f.ui.snapshot.inventory.buses[0].outputs[0].binding_revision++;if(reason==='offline')f.ui.client.views.ready=false;if(reason==='address')f.ui.snapshot.inventory.buses[0].outputs[0].short_address=1;if(reason==='disabled')f.ui.snapshot.inventory.buses[0].enabled=false;return f.report();};
    await assert.rejects(f.scope.read(),undefined,'retired output scope accepted a late read');assert.equal(f.scope.values,null);await assert.rejects(f.scope.save({minimum_level:5}));assert.equal(f.calls.length,1);f.ui.client.close();
  }
});
test('output settings failed or inconsistent writes retire actionable values until explicit refresh',async()=>{
  for(const reason of ['lost','outcome','match','generation','root','packed','reading']){
    const f=fixture();await f.scope.read();f.ui.client.job=async()=>{const r=f.report('configure_output',{...values,minimum_level:5});if(reason==='lost')throw new Error('Reply lost');if(reason==='outcome')r.outcome='unconfirmed';if(reason==='match'){r.settings.minimum_level=6;r.readings.minimum_level={kind:'value',raw:6};}if(reason==='generation')r.target_generation='other';if(reason==='root')f.ui.snapshot.inventory.revision++;if(reason==='packed')r.settings.fade_time=1;if(reason==='reading')r.readings.fade_time_rate={kind:'mask',raw:255};return r;};
    await assert.rejects(f.scope.save({minimum_level:5}),undefined,'output settings accepted mismatched or retired write evidence');assert.equal(f.scope.values,null,'failed settings reply retained actionable values');assert.equal(f.scope.result,null);assert.equal(f.ui.notice,'');await assert.rejects(f.scope.save({minimum_level:5}));f.ui.client.close();
  }
});
test('output settings matching identification blocks saves while allowing reads',async()=>{
  const f=fixture();Object.assign(f.ui.snapshot,{identification:{target:f.row.item.target,phase:'blinking'}});await f.scope.read();await assert.rejects(f.scope.save({minimum_level:5}),/Stop Blink/);assert.equal(f.calls.length,1);f.ui.client.close();
});
