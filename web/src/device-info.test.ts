import test from 'node:test';
import assert from 'node:assert/strict';
import {Controller} from './controller.ts';
import type {Snapshot,PhysicalRow} from './model.ts';
import {DeviceInfoScope,decodeMemory,memoryWindow} from './device-info.ts';
import {readFileSync} from 'node:fs';
import {luminaireDisplay} from './luminaire.ts';
function fixture(){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef',target='Bus1.Outputs.Lamp';
  ui.client.views.ready=true;ui.client.views.value={phase:'ready',identity:{controller:'Controller'},hardware:{state:'ready',buses:[{bus:0,status:2}]},states:[],inventory:{name:'Controller',revision:1,inventory_epoch:epoch,buses:[{bus:0,name:'Bus1',enabled:true,outputs:[{name:'Lamp',target,binding_revision:1,short_address:0}]}],identity_ledger:{baseline:[{target,binding_revision:1,location:{kind:'output',bus:0,short_address:0}}],batches:[]}}} as unknown as Snapshot;
  const row=ui.currentRows[0] as PhysicalRow,scope=new DeviceInfoScope(ui,row),calls:any[]=[];
  const report=()=>({operation:'read_memory',outcome:'observed',target,target_generation:row.generation,revision:1,bus:0,address:0,space:'outputs',bank:255,start:0,count:3,quality:'partial',version:null,identity:null,reason:null,configuration_changed:false,readings:[{address:0,reading:{kind:'value',raw:255}},{address:1,reading:{kind:'no_response'}},{address:2,reading:{kind:'value',raw:0}}]});
  ui.client.job=async(g,t,c,p)=>{calls.push({generation:g,target:t,command:c,params:p});return report();};
  return {ui,row,scope,calls,report};
}
test('memory windows include bank255/location254 and reject wrap, fractional or unbounded reads',()=>{
  assert.deepEqual(memoryWindow(255,254,1),{bank:255,start:254,count:1});assert.doesNotThrow(()=>memoryWindow(0,191,64));
  for(const args of [[0,254,2],[0,255,1],[0,0,0],[0,0,65],[256,0,1],[-1,0,1],[0,1.5,1],[0,0,NaN]])assert.throws(()=>memoryWindow(...args as [number,number,number]));
});
test('memory read preserves numeric FF and zero without shifting holes',async()=>{
  const f=fixture();const r=await f.scope.read({bank:255,start:0,count:3});assert.deepEqual(r,f.report());
  assert.deepEqual(f.calls,[{generation:f.row.generation,target:f.row.item.target,command:'read_memory',params:{bank:255,start:0,count:3}}]);
  const invalid=[{quality:'complete'},{count:2},{bank:0},{version:{kind:'value',raw:8}},{readings:[{address:0,reading:{kind:'mask',raw:255}},{address:1,reading:{kind:'no_response'}},{address:2,reading:{kind:'value',raw:0}}]}];
  for(const patch of invalid)assert.throws(()=>decodeMemory({...f.report(),...patch},'read_memory',{bank:255,start:0,count:3}));f.ui.client.close();
});
function identity(){
  const f=fixture(),bytes=[26,0,1,0,1,2,3,4,5,6,7,254,220,186,152,118,84,50,16,2,3,0,0,0,0,0,0];
  return {...f.report(),operation:'read_identity',bank:0,count:27,quality:'complete',version:{kind:'value',raw:8},readings:bytes.map((raw,address)=>({address,reading:{kind:'value',raw}})),identity:{layout:'modern',gtin:'00004328719365',serial_number:'FEDCBA9876543210',serial_bits:64,firmware_version:[6,7],hardware_version:[2,3],last_memory_bank:1}};
}
test('identity preserves exact-width serials and unknown version never invents a layout',()=>{
  const r=identity();assert.equal(decodeMemory(r,'read_identity').identity!.serial_number,'FEDCBA9876543210');
  for(const patch of [{serial_number:18364758544493064720},{serial_number:'FEDCBA987654321'},{serial_bits:32},{layout:'legacy'},{firmware_version:[1,256]}])assert.throws(()=>decodeMemory({...r,identity:{...r.identity,...patch}},'read_identity'));
  const unknown={...r,count:11,quality:'partial',version:{kind:'value',raw:7},readings:r.readings.slice(0,11),identity:{...r.identity,layout:'unknown',serial_number:null,serial_bits:null,hardware_version:null}};
  assert.doesNotThrow(()=>decodeMemory(unknown,'read_identity'));assert.throws(()=>decodeMemory({...unknown,quality:'complete'},'read_identity'));
});
test('identity missing or reserved header is unavailable or partial without fabricated fields',()=>{
  const r=identity();
  assert.doesNotThrow(()=>decodeMemory({...r,count:1,quality:'unavailable',identity:null,readings:[{address:0,reading:{kind:'no_response'}}]},'read_identity'));
  assert.doesNotThrow(()=>decodeMemory({...r,count:1,quality:'partial',identity:null,readings:[{address:0,reading:{kind:'value',raw:255}}]},'read_identity'));
});
test('memory late reads retire the original dialog and leave no previous result',async()=>{
  for(const reason of ['cancel','epoch','root','generation','offline','address','disabled','target']){
    const f=fixture();await f.scope.read({bank:255,start:0,count:3});
    f.ui.client.job=async()=>{const r=f.report();if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='root')f.ui.snapshot.inventory.revision++;if(reason==='generation')f.ui.snapshot.inventory.buses[0].outputs[0].binding_revision++;if(reason==='offline')f.ui.client.views.ready=false;if(reason==='address')f.ui.snapshot.inventory.buses[0].outputs[0].short_address=1;if(reason==='disabled')f.ui.snapshot.inventory.buses[0].enabled=false;if(reason==='target')r.target='Bus1.Outputs.Other';return r;};
    await assert.rejects(f.scope.read({bank:255,start:0,count:3}),undefined,`late ${reason} read populated information`);assert.equal(f.scope.result,null);f.ui.client.close();
  }
});
test('memory lost read clears previous evidence and invalid ranges submit no command',async()=>{
  const f=fixture();await f.scope.read({bank:255,start:0,count:3});f.ui.client.job=async()=>{throw new Error('Reply lost');};
  await assert.rejects(f.scope.read({bank:255,start:0,count:3}),/Reply lost/);assert.equal(f.scope.result,null);
  await assert.rejects(f.scope.read({bank:255,start:254,count:3}));assert.equal(f.calls.length,1);f.ui.client.close();
});

// Producer profile/serialized grammar: service::tests::luminaire's native MQTT case.
function luminaire(f:ReturnType<typeof fixture>){
  const profile=JSON.parse(readFileSync(new URL('../../examples/emulator/luminaire-i4.json',import.meta.url),'utf8'));
  const bytes=profile.buses[0].outputs[0].memory.banks[1].bytes;
  const fields=Object.fromEntries([
    ['manufacturer_gtin','00004328719365',null],['luminaire_id','FEDCBA9876543210',null],['manufacture_year',24,null],['manufacture_week',53,null],
    ['input_power_nominal',42,'W'],['input_power_minimum_dim',7,'W'],['mains_voltage_minimum',230,'V'],['mains_voltage_maximum',480,'V'],
    ['light_output_nominal',1000,'lm'],['cri',95,null],['cct',3000,'K'],['light_distribution','type_v',null],['oem_color','Black',null],['identification','Garden fixture',null]
  ].map(([key,value,unit])=>[key,{status:'value',value,unit}]));
  return {...f.report(),operation:'read_luminaire_data',bank:1,start:0,count:120,quality:'complete',readings:bytes.map((raw:number,address:number)=>({address,reading:{kind:'value',raw}})),luminaire:{format_id:3,format_status:'supported',fields}};
}
test('luminaire scope reads the original fixture and preserves exact identifiers and units',async()=>{
  const f=fixture();f.ui.client.job=async(g,t,c,p)=>{f.calls.push({generation:g,target:t,command:c,params:p});return luminaire(f);};
  const r=await f.scope.readLuminaire();assert.equal(r.readings.length,120);assert.equal(r.luminaire!.fields.luminaire_id.value,'FEDCBA9876543210');
  assert.deepEqual(f.calls,[{generation:f.row.generation,target:f.row.item.target,command:'read_luminaire_data',params:{}}]);
  assert.equal(luminaireDisplay('input_power_nominal',r.luminaire!.fields.input_power_nominal),'42 W');assert.equal(luminaireDisplay('manufacture_year',{status:'value',value:3,unit:null}),'03');f.ui.client.close();
});
test('luminaire capture does not relax raw memory windows or format/header bounds',()=>{
  const f=fixture(),r=luminaire(f);assert.doesNotThrow(()=>decodeMemory(r,'read_luminaire_data'));assert.throws(()=>memoryWindow(1,0,120));
  for(const patch of [{bank:2},{space:'inputs'},{count:121},{identity:{}},{version:{kind:'value',raw:8}}])assert.throws(()=>decodeMemory({...r,...patch},'read_luminaire_data'));
  const wrong=structuredClone(r);wrong.readings[0].reading.raw=118;assert.throws(()=>decodeMemory(wrong,'read_luminaire_data'),'header ignored');
  const unknown=structuredClone(r);unknown.count=19;unknown.quality='partial';unknown.readings=unknown.readings.slice(0,19);unknown.readings[18].reading.raw=4;unknown.luminaire={format_id:4,format_status:'unrecognized',fields:{}};
  assert.doesNotThrow(()=>decodeMemory(unknown,'read_luminaire_data'));assert.throws(()=>decodeMemory({...unknown,quality:'complete'},'read_luminaire_data'));assert.throws(()=>decodeMemory({...unknown,luminaire:r.luminaire},'read_luminaire_data'));f.ui.client.close();
});
test('luminaire known fields reject inconsistent status, precision, units and special CCT',()=>{
  const f=fixture(),r=luminaire(f);
  for(const [key,patch]of [['luminaire_id',{value:18364758544493064720}],['luminaire_id',{value:'FEDCBA987654321'}],['input_power_nominal',{unit:'V'}],['cri',{value:101}],['cct',{value:65534}],['identification',{value:'café'}],['oem_color',{status:'unknown'}]]){
    const wrong=structuredClone(r);Object.assign(wrong.luminaire.fields[key as string],patch);assert.throws(()=>decodeMemory(wrong,'read_luminaire_data'),undefined,`invalid luminaire field accepted: ${key}`);
  }
  const special=structuredClone(r);special.readings[33].reading.raw=255;special.readings[34].reading.raw=254;special.luminaire.fields.cct={status:'value',value:'part_209',unit:null};
  const decoded=decodeMemory(special,'read_luminaire_data');assert.equal(luminaireDisplay('cct',decoded.luminaire!.fields.cct),'Variable (Part 209 indication)');f.ui.client.close();
});
test('luminaire unavailable header keeps unknowns separate from a reserved header',()=>{
  const f=fixture(),r=luminaire(f),base={...r,count:1,quality:'unavailable',readings:[{address:0,reading:{kind:'no_response'}}],luminaire:{format_id:null,format_status:'unavailable',fields:{}}};
  assert.doesNotThrow(()=>decodeMemory(base,'read_luminaire_data'));
  assert.doesNotThrow(()=>decodeMemory({...base,quality:'partial',readings:[{address:0,reading:{kind:'value',raw:255}}]},'read_luminaire_data'));f.ui.client.close();
});
test('luminaire late and lost reads clear previous evidence without renewing scope',async()=>{
  for(const reason of ['cancel','epoch','revision','lost']){
    const f=fixture();f.ui.client.job=async()=>luminaire(f);await f.scope.readLuminaire();
    f.ui.client.job=async()=>{if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='revision')f.ui.snapshot.inventory.revision++;if(reason==='lost')throw new Error('Reply lost');return luminaire(f);};
    await assert.rejects(f.scope.readLuminaire(),undefined,`late ${reason} luminaire read populated dialog`);assert.equal(f.scope.result,null);f.ui.client.close();
  }
});
