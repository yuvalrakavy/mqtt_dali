import test from 'node:test';
import assert from 'node:assert/strict';
import {decodeDiagnostics,diagnosticBank,diagnosticDisplay,type DiagnosticBank} from './diagnostics.ts';
const proof={start:{kind:'value',raw:170},end:{kind:'value',raw:170},release:{kind:'value',raw:255}};
function fixture(bank:DiagnosticBank){
 const driver=[28,0,255,1,0x12,0x34,0x56,0x78,1,2,3,8,252,50,99,1,3,0,4,1,5,0,6,1,7,0,8,35,100];
 const source=[32,0,255,1,1,2,3,2,3,4,0,0,14,16,0x12,0x34,0x56,0x78,1,225,4,210,0,1,1,2,0,3,1,4,0,5,85];
 const rows=bank===205?[
  ['operating_time',305419896,'305419896','s'],['start_counter',66051,'66051','count'],['supply_voltage',2300,'230','Vrms'],['supply_frequency',50,'50','Hz'],['power_factor',99,'0.99',null],['overall_failure',1,true,null],['overall_failure_counter',3,'3','count'],['undervoltage',0,false,null],['undervoltage_counter',4,'4','count'],['overvoltage',1,true,null],['overvoltage_counter',5,'5','count'],['power_limit',0,false,null],['power_limit_counter',6,'6','count'],['thermal_derating',1,true,null],['thermal_derating_counter',7,'7','count'],['thermal_shutdown',0,false,null],['thermal_shutdown_counter',8,'8','count'],['temperature',35,'-25','°C'],['output_current_percent',100,'100','%']
 ]:bank===206?[
  ['resettable_start_counter',66051,'66051','count'],['start_counter',131844,'131844','count'],['resettable_on_time',3600,'3600','s'],['on_time',305419896,'305419896','s'],['voltage',481,'48.1','V'],['current',1234,'1.234','A'],['overall_failure',0,false,null],['overall_failure_counter',1,'1','count'],['short_circuit',1,true,null],['short_circuit_counter',2,'2','count'],['open_circuit',0,false,null],['open_circuit_counter',3,'3','count'],['thermal_derating',1,true,null],['thermal_derating_counter',4,'4','count'],['thermal_shutdown',0,false,null],['thermal_shutdown_counter',5,'5','count'],['temperature',85,'25','°C']
 ]:[['rated_life',50,'50000','h'],['reference_temperature',0,'-60','°C'],['rated_starts',65533,'6553300','count']];
 const bytes:(number|null)[]=bank===205?driver:bank===206?source:[7,0,255,1,50,0,255,253];
 const fields=Object.fromEntries(rows.map(([key,raw,value,unit])=>[key,{status:'value',raw,value,unit}]));
 const data={version:1,format_status:'supported',fields,snapshot:bank===207?null:structuredClone(proof)};
 return {data,bytes};
}
test('diagnostics decoder accepts all39 producer-shaped fields and exact boolean/number grammar',()=>{
 for(const bank of [205,206,207]as const){const f=fixture(bank);assert.deepEqual(decodeDiagnostics(f.data,bank,f.bytes),{data:f.data,partial:false});}
 const driver=decodeDiagnostics(fixture(205).data,205,fixture(205).bytes).data.fields;
 assert.equal(diagnosticDisplay(driver.temperature),'-25 °C');assert.equal(diagnosticDisplay(driver.overall_failure),'Yes');assert.equal(diagnosticDisplay(driver.undervoltage),'No');assert.equal(diagnosticDisplay(driver.power_factor),'0.99');
 for(const bank of [205,206,207])assert.equal(diagnosticBank(bank),bank);
 for(const bank of [0,204,208,255,205.5,NaN])assert.throws(()=>diagnosticBank(bank));
});
test('diagnostics decoder rejects fields inconsistent with raw bytes, units, reserved values or snapshot proof',()=>{
 const f=fixture(205);
 for(const [key,patch]of [['temperature',{value:'35'}],['temperature',{value:-25}],['overall_failure',{value:'true'}],['overall_failure',{raw:2}],['power_factor',{value:'0.990'}],['supply_voltage',{unit:'V'}],['undervoltage',{status:'unknown',value:null}],['operating_time',{value:'305419897'}]]as const){const wrong=structuredClone(f.data);Object.assign(wrong.fields[key],patch);assert.throws(()=>decodeDiagnostics(wrong,205,f.bytes),undefined,`invalid diagnostic field accepted:${key}`);}
 for(const mutate of [(r:any)=>r.snapshot=null,(r:any)=>r.snapshot.release.raw=170,(r:any)=>r.snapshot.start.raw=255,(r:any)=>delete r.fields.temperature,(r:any)=>r.fields.extra=r.fields.temperature]){const wrong=structuredClone(f.data);mutate(wrong);assert.throws(()=>decodeDiagnostics(wrong,205,f.bytes));}
 const stat=fixture(207);stat.data.snapshot=structuredClone(proof);assert.throws(()=>decodeDiagnostics(stat.data,207,stat.bytes));
});
test('diagnostics reserved and missing values remain distinct and temperatures use unsigned offset',()=>{
 const f=fixture(205);f.bytes[11]=255;f.bytes[12]=255;f.data.fields.supply_voltage={status:'read_protected',raw:65535,value:null,unit:'Vrms'};
 f.bytes[15]=255;f.data.fields.overall_failure={status:'invalid',raw:255,value:null,unit:null};
 f.bytes[16]=254;f.data.fields.overall_failure_counter={status:'temporarily_unavailable',raw:254,value:null,unit:'count'};
 f.bytes[27]=128;f.data.fields.temperature={status:'value',raw:128,value:'68',unit:'°C'};
 f.bytes[4]=null;f.data.fields.operating_time={status:'unavailable',raw:null,value:null,unit:'s'};
 assert.doesNotThrow(()=>decodeDiagnostics(f.data,205,f.bytes));
 for(const [key,text]of [['supply_voltage','Read protected'],['overall_failure','Invalid data'],['overall_failure_counter','Temporarily unavailable'],['operating_time','Unavailable']])assert.equal(diagnosticDisplay(f.data.fields[key]),text);
 const m=fixture(207);m.bytes[4]=255;m.data.fields.rated_life={status:'unknown',raw:255,value:null,unit:'h'};assert.doesNotThrow(()=>decodeDiagnostics(m.data,207,m.bytes));assert.equal(diagnosticDisplay(m.data.fields.rated_life),'Unknown');
 const wrong=structuredClone(f.data);wrong.fields.supply_voltage.status='unknown';assert.throws(()=>decodeDiagnostics(wrong,205,f.bytes));
});
test('diagnostics invalid booleans and percentages remain received evidence with no usable value',()=>{
 for(const[bank,key,offset,raw]of[[205,'overall_failure',15,2],[205,'power_factor',14,101],[205,'output_current_percent',28,101],[206,'overall_failure',22,2],[206,'short_circuit',24,2],[206,'open_circuit',26,2],[206,'thermal_derating',28,2],[206,'thermal_shutdown',30,2]]as const){
  const f=fixture(bank);f.bytes[offset]=raw;f.data.fields[key]={...f.data.fields[key],status:'invalid',raw,value:null};assert.doesNotThrow(()=>decodeDiagnostics(f.data,bank,f.bytes),`invalid raw code converted to a value:${key}`);
  const wrong=structuredClone(f.data);wrong.fields[key]={...wrong.fields[key],status:'value',value:key==='power_factor'?'1.01':key==='output_current_percent'?'101':false};assert.throws(()=>decodeDiagnostics(wrong,bank,f.bytes));
 }
 for(const[raw,value]of[[0,'-60'],[60,'0'],[128,'68'],[253,'193']]as const){const f=fixture(206);f.bytes[32]=raw;f.data.fields.temperature={status:'value',raw,value,unit:'°C'};assert.doesNotThrow(()=>decodeDiagnostics(f.data,206,f.bytes));}
});
test('diagnostics decoder validates header/version bounds and never accepts inferred layout or unlatched dynamic values',()=>{
 for(const bank of [205,206,207]as const){
  const f=fixture(bank);f.bytes[3]=2;const prefix=f.bytes.slice(0,4),unknown={version:2,format_status:'unrecognized',fields:{},snapshot:null};assert.deepEqual(decodeDiagnostics(unknown,bank,prefix),{data:unknown,partial:true});assert.throws(()=>decodeDiagnostics(unknown,bank,f.bytes));assert.throws(()=>decodeDiagnostics(f.data,bank,prefix));
  const absent={version:null,format_status:'unavailable',fields:{},snapshot:null};assert.equal(decodeDiagnostics(absent,bank,[null]).partial,false);assert.equal(decodeDiagnostics(absent,bank,[255]).partial,true);
  assert.throws(()=>decodeDiagnostics(absent,bank,[]));assert.throws(()=>decodeDiagnostics(absent,bank,[null,0]));
  f.bytes[3]=1;f.bytes[0]=3;for(const field of Object.values(f.data.fields)){field.status='unavailable';field.raw=null;field.value=null;}assert.equal(decodeDiagnostics(f.data,bank,f.bytes.slice(0,4)).partial,true);assert.throws(()=>decodeDiagnostics(f.data,bank,f.bytes));
 }
});
