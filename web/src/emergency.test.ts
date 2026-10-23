import test from 'node:test';
import assert from 'node:assert/strict';
import {Controller} from './controller.ts';
import {decodeEmergency,validateEmergencyPatch,EmergencyScope} from './emergency.ts';
import type {PhysicalRow,Snapshot} from './model.ts';
const modeNames=['rest','normal','emergency','extended_emergency','function_test','duration_test','hardwired_inhibit','hardwired_switch_on'];
const featureNames=['integral','maintained','switched_maintained','automatic_tests','adjustable_level','hardwired_inhibit','physical_selection','relight_in_rest'];
const failureNames=['circuit_failure','battery_duration_failure','battery_failure','emergency_lamp_failure','function_max_delay_exceeded','duration_max_delay_exceeded','function_test_failed','duration_test_failed'];
const statusNames=['inhibited','function_done_valid','duration_done_valid',null,'function_pending','duration_pending','identification_active','physically_selected'];
function native(){
  const readings:any={presence:{kind:'value',raw:4},device_type:{kind:'value',raw:1},closing_presence:{kind:'value',raw:4}};
  const fields:any={};
  for(const [name,raw,value,unit,at_least]of [
    ['battery_charge',254,254,'raw',false],['last_duration_seconds',120,14400,'seconds',false],
    ['emergency_runtime_hours',255,255,'hours',true],['total_runtime_hours',255,1020,'hours',true],
    ['emergency_level',100,100,'raw',false],['minimum_level',1,1,'raw',false],['maximum_level',200,200,'raw',false],
    ['rated_duration_seconds',90,10800,'seconds',false],['mode_raw',2,2,'raw',false],['features_raw',27,27,'raw',false],
    ['failures_raw',0,0,'raw',false],['status_raw',6,6,'raw',false],['extended_version',1,1,'raw',false],
    ['function_test_interval_days',7,7,'days',false],['duration_test_interval_weeks',4,4,'weeks',false],
    ['test_execution_timeout_minutes',0,15,'minutes',false],['prolong_seconds',3,90,'seconds',false],
  ]as const){fields[name]={raw,status:'value',value,unit,at_least};readings[name]={kind:'value',raw};}
  for(const[name,word,value]of [['next_function_test_minutes',32,480],['next_duration_test_minutes',64,960]]as const){fields[name]={raw:word,status:'value',value,unit:'minutes',at_least:false};readings[name+'_high']={kind:'value',raw:0};readings[name+'_low']={kind:'value',raw:word};}
  const flags=(raw:number,names:(string|null)[])=>Object.fromEntries(names.flatMap((name,bit)=>name?[[name,!!(raw&(1<<bit))]]:[]));
  const observed={fields,mode:flags(2,modeNames),features:flags(27,featureNames),failures:flags(0,failureNames),status:flags(6,statusNames),quality:'complete'};
  return {operation:'read_emergency',outcome:'observed',target:'Bus1.Outputs.Lamp',target_generation:'0123456789abcdef0123456789abcdef.1',revision:1,bus:0,address:0,support:true,before:null,before_readings:null,observed,readings,reason:null,configuration_changed:false};
}
function fixture(){
  const ui=new Controller(),output={name:'Lamp',target:'Bus1.Outputs.Lamp',binding_revision:1,short_address:0};
  ui.client.views.ready=true;ui.client.views.value={phase:'ready',identity:{controller:'Controller'},hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',revision:1,inventory_epoch:'0123456789abcdef0123456789abcdef',buses:[{bus:0,name:'Bus1',enabled:true,outputs:[output]}],identity_ledger:{baseline:[{target:output.target,binding_revision:1,location:{kind:'output',bus:0,short_address:0}}],batches:[]}}}as unknown as Snapshot;
  const row=ui.currentRows[0]as PhysicalRow,scope=new EmergencyScope(ui,row),calls:any[]=[];
  ui.client.job=async(g,t,c,p)=>{calls.push({g,t,c,p});const r=native();r.operation=c;return r;};return {ui,row,scope,calls};
}
test('emergency report recomputes all nineteen fields and native flag maps',()=>{
  const report=native();assert.deepEqual(decodeEmergency(report),report);assert.equal(Object.keys(report.observed.fields).length,19);
  const unknown=native();unknown.readings.battery_charge.raw=255;Object.assign(unknown.observed.fields.battery_charge,{raw:255,value:null,status:'unknown'});assert.deepEqual(decodeEmergency(unknown),unknown);
});
test('emergency report refuses invented support, altered units, saturation, flags or atomic bytes',()=>{
  for(const mutate of [(r:any)=>r.support=false,(r:any)=>r.observed.fields.total_runtime_hours.value=1016,(r:any)=>r.observed.fields.emergency_runtime_hours.at_least=false,(r:any)=>r.readings.next_function_test_minutes_low.raw=33,(r:any)=>r.observed.fields.prolong_seconds.unit='minutes',(r:any)=>r.observed.features.adjustable_level=false,(r:any)=>delete r.readings.status_raw,(r:any)=>r.readings.battery_charge={kind:'mask',raw:255},(r:any)=>r.observed.status.battery_fully_charged=true]){
    const report=native();mutate(report);assert.throws(()=>decodeEmergency(report),undefined,'inconsistent emergency native evidence accepted');
  }
});
test('emergency patches require fresh features, checked native units and paired initial schedules',()=>{
  const values=native().observed;
  validateEmergencyPatch({emergency_level:150,prolong_seconds:7650,test_execution_timeout_minutes:15,function_test_interval_days:7,function_delay_minutes:480,duration_test_interval_weeks:97,duration_delay_minutes:983010},values);
  for(const patch of [{},{emergency_level:0},{emergency_level:201},{prolong_seconds:1},{test_execution_timeout_minutes:16},{function_test_interval_days:7},{function_delay_minutes:480},{function_test_interval_days:0,function_delay_minutes:0},{duration_test_interval_weeks:98,duration_delay_minutes:15},{prolong_seconds:null},{surprise:1}])assert.throws(()=>validateEmergencyPatch(patch as any,values));
  const fixed=structuredClone(values);fixed.features.adjustable_level=false;assert.throws(()=>validateEmergencyPatch({emergency_level:100},fixed));
});
test('emergency scope keeps original authority and sends prompt explicit test requests',async()=>{
  const f=fixture();await f.scope.read();
  f.ui.client.job=async(g,t,c,p)=>{f.calls.push({g,t,c,p});const r=native();r.operation=c;r.outcome=c==='configure_emergency'?'applied':'request_sent';if(c==='configure_emergency'){r.readings.prolong_seconds.raw=4;Object.assign(r.observed.fields.prolong_seconds,{raw:4,value:120});}return r;};
  await f.scope.save({prolong_seconds:120});await f.scope.test('function');await f.scope.test('duration');await f.scope.test(null);
  assert.deepEqual(f.calls.slice(1).map(c=>[c.g,c.t,c.c,c.p]),[
    [f.row.generation,f.row.item.target,'configure_emergency',{expected_revision:1,settings:{prolong_seconds:120}}],
    [f.row.generation,f.row.item.target,'start_emergency_test',{expected_revision:1,test:'function'}],
    [f.row.generation,f.row.item.target,'start_emergency_test',{expected_revision:1,test:'duration'}],
    [f.row.generation,f.row.item.target,'stop_emergency_test',{expected_revision:1}],
  ]);f.ui.client.close();
});
test('emergency late, lost and unconfirmed replies clear actionable values without replay',async()=>{
  for(const reason of ['cancel','epoch','revision','generation','address','disabled','offline','lost','unconfirmed']){
    const f=fixture();await f.scope.read();f.ui.client.job=async()=>{
      f.calls.push('write');const r=native();r.operation='configure_emergency';r.outcome='applied';
      if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='revision')f.ui.snapshot.inventory.revision++;if(reason==='generation')f.ui.snapshot.inventory.buses[0].outputs[0].binding_revision++;if(reason==='address')f.ui.snapshot.inventory.buses[0].outputs[0].short_address=1;if(reason==='disabled')f.ui.snapshot.inventory.buses[0].enabled=false;if(reason==='offline')f.ui.client.views.ready=false;if(reason==='lost')throw new Error('Reply lost');if(reason==='unconfirmed')r.outcome='unconfirmed';return r;
    };
    await assert.rejects(f.scope.save({prolong_seconds:120}));assert.equal(f.scope.values,null);assert.equal(f.scope.result,null);await assert.rejects(f.scope.test('duration'));assert.equal(f.calls.length,2);f.ui.client.close();
  }
});
test('emergency unsupported or identifying fixture cannot authorize native writes',async()=>{
  const f=fixture();f.ui.client.job=async()=>({...native(),support:false,observed:null,readings:{presence:{kind:'value',raw:4},device_type:{kind:'value',raw:7}}});await f.scope.read();assert.equal(f.scope.values,null);await assert.rejects(f.scope.test('function'));await assert.rejects(f.scope.save({prolong_seconds:0}));
  f.ui.client.job=async()=>native();await f.scope.read();Object.assign(f.ui.snapshot,{identification:{target:f.row.item.target,phase:'blinking'}});await assert.rejects(f.scope.test(null),/Stop Blink/);f.ui.client.close();
});
