import test from 'node:test';
import assert from 'node:assert/strict';
import {Controller} from './controller.ts';
import {GroupOutputScope} from './group-output.ts';
import type {Snapshot,PhysicalRow} from './model.ts';

function fixture(){
  const ui=new Controller(),epoch='0123456789abcdef0123456789abcdef';
  const outputs=[{name:'First',target:'Bus2.Outputs.First',binding_revision:1,short_address:0},{name:'Last',target:'Bus2.Outputs.Last',binding_revision:1,short_address:63},{name:'Outside',target:'Bus2.Outputs.Outside',binding_revision:1,short_address:1}];
  const group={name:'Lights',target:'Bus2.Groups.Lights',binding_revision:1,group_address:15,members:[0,63]};
  const baseline=[...outputs.map(o=>({target:o.target,binding_revision:1,location:{kind:'output',bus:1,short_address:o.short_address}})),{target:group.target,binding_revision:1,location:{kind:'group',bus:1,group_address:15}}];
  ui.client.views.ready=true;ui.client.views.value={phase:'ready',identity:{controller:'Controller'},hardware:{state:'ready',buses:[{bus:1,status:2}]},states:[],inventory:{name:'Controller',revision:1,inventory_epoch:epoch,buses:[{bus:1,name:'Bus2',enabled:true,outputs,groups:[group]}],identity_ledger:{baseline,batches:[]}}} as unknown as Snapshot;
  const row=ui.currentRows.find(r=>r.kind==='group') as PhysicalRow,scope=new GroupOutputScope(ui,row),calls:any[]=[];
  const value=(raw:number)=>({kind:'value',raw});
  const report=(operation='read_group_output',levels=[100,100])=>({operation,outcome:operation==='read_group_output'?'observed':'sent',target:row.item.target,target_generation:row.generation,revision:1,bus:1,group:15,coverage:'saved_inventory',configuration_changed:false,membership_matches:true,
    members:outputs.map((o,i)=>({target:o.target,target_generation:row.generation,address:o.short_address,expected_member:i<2,member:i<2,membership_low:value(0),membership_high:value(i<2?128:0),actual_level:i<2?value(levels[i]):null,status:i<2?value(255):null})),
    values:{quality:levels[0]===levels[1]?'uniform':'mixed',power_state:(levels[0]>0)===(levels[1]>0)?levels[0]>0:null,brightness:levels[0]===levels[1]?levels[0]/254:null,power_mixed:(levels[0]>0)!==(levels[1]>0),brightness_mixed:levels[0]!==levels[1],known_members:2,unresolved:[]}});
  ui.client.job=async(generation,target,command,params)=>{calls.push({generation,target,command,params});return report(command);};
  return {ui,row,scope,calls,report,value};
}
test('group output preserves native scope and original authority and shows actual values',async()=>{
  const f=fixture(),saved=structuredClone(f.ui.snapshot);assert.equal((await f.scope.read()).values.power_state,true);
  f.ui.client.job=async(generation,target,command,params:any)=>{f.calls.push({generation,target,command,params});return f.report(command,[0,0]);};
  assert.equal((await f.scope.set({power:false})).values.power_state,false);
  assert.deepEqual(f.calls[1],{generation:f.row.generation,target:f.row.item.target,command:'set_group_output',params:{expected_revision:1,power:false}});
  assert.match(f.ui.notice,/command sent/);assert.deepEqual(f.ui.snapshot,saved);f.ui.client.close();
});
test('group output distinguishes mixed levels from mixed power and never averages',async()=>{
  for(const levels of [[1,254],[0,254]]){const f=fixture();f.ui.client.job=async()=>f.report('read_group_output',levels);const r=await f.scope.read();assert.equal(r.values.brightness,null);assert.equal(r.values.power_state,levels[0]?true:null);assert.equal(r.values.power_mixed,!levels[0]);f.ui.client.close();}
});
test('group output incomplete roster and empty groups do not fabricate off',async()=>{
  const f=fixture();const report=f.report();Object.assign(report.members[1],{member:null,membership_low:null,membership_high:null,actual_level:null,status:null});
  Object.assign(report.values,{quality:'partial',power_state:null,brightness:null,power_mixed:null,brightness_mixed:null,known_members:1,unresolved:[63]});report.membership_matches=null as any;
  f.ui.client.job=async()=>report;assert.equal((await f.scope.read()).values.power_state,null);
  // Explicit writes remain valid without usable previous aggregate state.
  f.ui.client.job=async(_g,_t,command)=>f.report(command,[0,0]);assert.equal((await f.scope.set({power:false})).values.power_state,false);
  const empty=f.report();for(const m of empty.members){Object.assign(m,{member:false,membership_high:f.value(0),actual_level:null,status:null});}
  Object.assign(empty.values,{quality:'empty',power_state:null,brightness:null,power_mixed:null,brightness_mixed:null,known_members:0});empty.membership_matches=false;
  f.ui.client.job=async()=>empty;assert.equal((await f.scope.read()).values.power_state,null);f.ui.client.close();
});
test('group output rejects forged aggregate values incomplete rosters and wrong physical identity',async()=>{
  for(const defect of ['power','brightness','quality','mixed','count','unresolved','row_missing','duplicate','unexpected','member','expected','row_generation','row_target','mask','status','operation','outcome','target','generation','revision','bus','group','coverage','config','membership']){
    const f=fixture(),r:any=f.report();
    if(defect==='power')r.values.power_state=false;if(defect==='brightness')r.values.brightness=1;if(defect==='quality')r.values.quality='partial';if(defect==='mixed')r.values.power_mixed=true;
    if(defect==='count')r.values.known_members=1;if(defect==='unresolved')r.values.unresolved=[63];if(defect==='row_missing')r.members.pop();if(defect==='duplicate')r.members[1]=r.members[0];
    if(defect==='unexpected')r.members[1].address=5;if(defect==='member')r.members[1].member=false;if(defect==='expected')r.members[1].expected_member=false;
    if(defect==='row_generation')r.members[1].target_generation='new';if(defect==='row_target')r.members[1].target='new';if(defect==='mask')r.members[1].actual_level={kind:'mask',raw:255};if(defect==='status')r.members[1].status={kind:'receive_collision',code:7};
    if(defect==='operation')r.operation='set_group_output';if(defect==='outcome')r.outcome='sent';if(defect==='target')r.target='new';if(defect==='generation')r.target_generation='new';
    if(defect==='revision')r.revision=2;if(defect==='bus')r.bus=0;if(defect==='group')r.group=0;if(defect==='coverage')r.coverage='entire_bus';if(defect==='config')r.configuration_changed=true;if(defect==='membership')r.membership_matches=false;
    f.ui.client.job=async()=>r;await assert.rejects(f.scope.read(),undefined,`forged group result accepted: ${defect}`);assert.equal(f.scope.result,null);f.ui.client.close();
  }
});
test('group output retains original scope across awaits and cancellation',async()=>{
  for(const reason of ['cancel','epoch','root','generation','slot','offline','logout']){
    const f=fixture();f.ui.client.job=async()=>{f.calls.push('read');if(reason==='cancel')f.scope.cancelled=true;if(reason==='epoch')f.ui.client.views.epoch++;if(reason==='root')f.ui.snapshot.inventory.revision++;if(reason==='generation')f.ui.snapshot.inventory.buses[0].groups![0].binding_revision++;if(reason==='slot')f.ui.snapshot.inventory.buses[0].groups![0].group_address=0;if(reason==='offline')f.ui.client.views.ready=false;if(reason==='logout')f.ui.signingOut=true;return f.report();};
    await assert.rejects(f.scope.read(),undefined,'retired group result was accepted');assert.equal(f.scope.result,null);await assert.rejects(f.scope.set({power:true}));assert.equal(f.calls.length,1,'retired group scope sent an action');f.ui.client.close();
  }
});
test('group output invalid intent refuses before any job',async()=>{
  for(const patch of [{},{power:null},{power:1},{power:'true'},{brightness:null},{brightness:NaN},{brightness:Infinity},{brightness:-0.1},{brightness:1.1},{power:true,brightness:0},{power:false,brightness:0.5},{power:true,unknown:1}]){
    const f=fixture();await assert.rejects(f.scope.set(patch));assert.equal(f.calls.length,0,'invalid group intent submitted a job');f.ui.client.close();
  }
});
test('group output uncertain replies clear prior evidence and sent state is actual not intent',async()=>{
  for(const failure of ['transport','unconfirmed','invalid']){
    const f=fixture();await f.scope.read();f.ui.client.job=async()=>{if(failure==='transport')throw new Error('lost reply');const r:any=f.report('set_group_output');if(failure==='unconfirmed'){r.outcome='unconfirmed';r.values=null;}else r.values.power_state=false;return r;};
    await assert.rejects(f.scope.set({power:false}));assert.equal(f.scope.result,null,'failed group operation retained current evidence');assert.equal(f.ui.notice,'');f.ui.client.close();
  }
  const f=fixture();assert.equal((await f.scope.set({power:false})).values.power_state,true,'group command echoed requested power instead of actual');f.ui.client.close();
});
