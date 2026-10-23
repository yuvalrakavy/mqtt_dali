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

const relaySettings={features_raw:136,adjustable_thresholds:true,physical_selection:true,switch_status_raw:255,gear_type_raw:226,extended_version_raw:1,upward_on:50,upward_off:255,downward_on:255,downward_off:0,quality:'complete'};
const relayValues=()=>({...values,physical_minimum:254,minimum_level:1,switching_support:true,switching:structuredClone(relaySettings)}) as OutputValues&any;
function relayReport(f:ReturnType<typeof fixture>,command='read_output_settings',v=relayValues()){
  const r=f.report(command,v) as any;r.readings.device_type={kind:'value',raw:7};
  for(const [name,field]of Object.entries({switching_features:'features_raw',switching_status:'switch_status_raw',switching_gear_type:'gear_type_raw',switching_version:'extended_version_raw',switching_upward_on:'upward_on',switching_upward_off:'upward_off',switching_downward_on:'downward_on',switching_downward_off:'downward_off'}))r.readings[name]=v.switching?.[field]===null?{kind:'no_response'}:{kind:'value',raw:v.switching?.[field]};
  return r;
}
test('DT7 settings allow successive virtual limits only with proved adjustable support',()=>{
  const v=relayValues();assert.doesNotThrow(()=>decodeOutputValues(v),'DT7 virtual limits were mistaken for invalid physical limits');assert.deepEqual(decodeOutputValues(v),v);
  assert.equal(outputWritable('minimum_level',v),true,'DT7 existing virtual limit cannot be edited again');
  validateOutputPatch({minimum_level:5,maximum_level:200},v);
  const after=structuredClone(v);after.switching.upward_on=255;after.switching.downward_off=1;
  assert.deepEqual(changedOutputSettings(v,after),{switching:{upward_on:255,downward_off:1}},'DT7 changed-only patch lost or rewrote a threshold');
  for(const patch of [{switching:{}},{switching:null},{switching:{upward_on:0}},{switching:{upward_off:null}},{switching:{downward_on:256}},{switching:{downward_off:1.5}},{switching:{surprise:1}}])assert.throws(()=>validateOutputPatch(patch as any,v),undefined,'DT7 malformed nested patch accepted');
  const fixed={...v,minimum_level:254,switching:{...v.switching,features_raw:128,adjustable_thresholds:false}};
  assert.throws(()=>validateOutputPatch({minimum_level:1},fixed),/physical minimum/);
  assert.throws(()=>validateOutputPatch({switching:{upward_on:55}} as any,fixed),undefined,'fixed relay threshold became writable');
});
test('DT7 settings require raw type and threshold agreement before accepting capabilities',async()=>{
  for(const mode of ['wrong_type','empty_tail','one_tail','duplicate','raw_threshold','raw_features','valid_prefix']){
    const f=fixture(),r=relayReport(f);if(mode==='wrong_type')r.readings.device_type.raw=6;
    if(['empty_tail','one_tail','duplicate','valid_prefix'].includes(mode)){
      r.readings.device_type.raw=255;const prefix=mode==='empty_tail'?[254]:mode==='one_tail'?[6,254]:mode==='duplicate'?[6,6]:[0,6,7];prefix.forEach((raw,index)=>r.readings[`device_type_${index}`]={kind:'value',raw});
    }
    if(mode==='raw_threshold')r.readings.switching_downward_off.raw=255;
    if(mode==='raw_features')r.readings.switching_features.raw=128;
    f.ui.client.job=async()=>r;
    if(mode==='valid_prefix'){await f.scope.read();assert.equal((f.scope.values as any).switching.downward_off,0);}else await assert.rejects(f.scope.read(),undefined,`DT7 accepted inconsistent capability evidence: ${mode}`);
    f.ui.client.close();
  }
});
test('DT7 settings unknown malformed tails and known absence remain independent of common quality',async()=>{
  for(const [prefix,support]of [[[254],null],[[6,254],null],[[0,6,254],false]] as const){
    const f=fixture(),v={...values,switching_support:support},r=f.report('read_output_settings',v) as any;
    r.readings.device_type={kind:'value',raw:255};prefix.forEach((raw,index)=>r.readings[`device_type_${index}`]={kind:'value',raw});f.ui.client.job=async()=>r;
    await f.scope.read();assert.equal(f.scope.values!.quality,'complete');assert.equal((f.scope.values as any).switching_support,support);f.ui.client.close();
  }
});
test('DT7 settings match every requested nested value and retire values after possible writes',async()=>{
  for(const mismatch of [false,true]){
    const f=fixture();f.ui.client.job=async()=>relayReport(f);await f.scope.read();
    f.ui.client.job=async()=>{const v=relayValues();v.switching.upward_on=mismatch?50:55;return relayReport(f,'configure_output',v);};
    if(mismatch){await assert.rejects(f.scope.save({switching:{upward_on:55}} as any),undefined,'DT7 requested threshold mismatch was accepted');assert.equal(f.scope.values,null);}
    else{await f.scope.save({switching:{upward_on:55}} as any);assert.equal((f.scope.values as any).switching.upward_on,55);}
    f.ui.client.close();
  }
});
