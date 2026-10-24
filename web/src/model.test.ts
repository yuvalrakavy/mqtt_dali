import test from 'node:test';
import assert from 'node:assert/strict';
import { glob, rowOrigins, snapshotOrigins, itemGeneration, inputDisplay, Draft, Views, rows } from './model.ts';
test('compact snapshots preserve draft keys after arbitrarily many offline renames and reject replacements',()=>{
  const epoch='0123456789abcdef0123456789abcdef',origin={target:'Bus.Outputs.Light1',revision:1};
  const initial={origin,projection:{target:origin.target,binding_revision:1,location:{kind:'output',bus:0,short_address:0}},intent_revision:1};
  const next={...initial,projection:{...initial.projection,target:'Bus.Outputs.Kitchen',binding_revision:1201}};
  const first=snapshotOrigins([initial],epoch,1),later=snapshotOrigins([next],epoch,1201);
  assert.equal(later.get(next.projection.target),first.get(origin.target));
  const replacement={...next,origin:{target:next.projection.target,revision:1202},intent_revision:1202,projection:{...next.projection,binding_revision:1202}};
  assert.notEqual(snapshotOrigins([replacement],epoch,1202).get(replacement.projection.target),later.get(next.projection.target));
  assert.throws(()=>snapshotOrigins([next,next],epoch,1201),/Duplicate/);
  const snapshot:any={inventory:{inventory_epoch:epoch,revision:1201,buses:[{bus:0,name:'Bus',enabled:true,outputs:[{name:'Kitchen',short_address:0,target:next.projection.target,binding_revision:1201}]}],identity_snapshot:[next]}};
  assert.equal(rows(snapshot)[0].key,later.get(next.projection.target),'actual row rendering still requires rename history');
});
test('query input display preserves full-resolution raw bits and does not invent sensor units',()=>{
  assert.equal(inputDisplay({InputValueRaw:'FEDCBA9876543210',ValueResolution:64}),'0xFEDCBA9876543210 / 64 bit','queried sensor value lost exact raw bits');
  assert.equal(inputDisplay({InputValueRaw:'0FFF',ValueResolution:12,Position:1}),'100% · 0x0FFF / 12 bit');
  assert.equal(inputDisplay({LightLevelRaw:711}),'711 raw');
  assert.equal(inputDisplay({InputValueRaw:null,LightLevelRaw:null}),'Unknown');
});

const p=(target:string,revision:number,address=0)=>({target,binding_revision:revision,location:{kind:'output',bus:0,short_address:address},members:[]});
test('origin replay applies rename swaps atomically and replacement retires the old draft key',()=>{
  const a=p('Bus.Outputs.A',1),b=p('Bus.Outputs.B',1,1),nextA=p('Bus.Outputs.B',2),nextB=p('Bus.Outputs.A',2,1);
  const initial=rowOrigins({baseline:[a,b],batches:[]},'epoch');
  const swapped=rowOrigins({baseline:[a,b],batches:[{revision:2,transitions:[{from:a,to:nextA},{from:b,to:nextB}]}]},'epoch');
  assert.equal(swapped.get(nextA.target),initial.get(a.target),'continuing rename lost its stable row key');
  assert.equal(swapped.get(nextB.target),initial.get(b.target),'atomic rename swap reused an occupied target');
  const replacement=p(nextA.target,3);
  const replaced=rowOrigins({baseline:[a,b],batches:[{revision:2,transitions:[{from:a,to:nextA},{from:b,to:nextB}]},{revision:3,transitions:[{from:nextA,to:null},{from:null,to:replacement}]}]},'epoch');
  assert.notEqual(replaced.get(replacement.target),swapped.get(nextA.target),'same-address/name replacement inherited a retired row key');
});
test('name glob is literal apart from star and question mark and does not interpret regex',()=>{
  assert.equal(glob('light*','Light12'),true);assert.equal(glob('Input?','input2'),true);
  assert.equal(glob('Kitchen[1]','Kitchen1'),false);assert.equal(glob('Kitchen[1]','Kitchen[1]'),true);
  assert.equal(glob('','anything'),true);
});
test('authority refuses unsafe binding revisions rather than rounding',()=>{
  assert.equal(itemGeneration('0123456789abcdef0123456789abcdef',3),'0123456789abcdef0123456789abcdef.3');
  assert.throws(()=>itemGeneration('0123456789abcdef0123456789abcdef',Number.MAX_SAFE_INTEGER+1),undefined,'unsafe metadata revision was accepted');
});
test('checkmark and blur share one commit and preserve the baseline while live values change',async()=>{
  const draft=new Draft('Light1','Bus.Outputs.Light1','epoch.1');draft.text='Kitchen';
  let writes=0;let release!:()=>void;const held=new Promise<void>(r=>release=r);
  const first=draft.commit(async(value,baseline)=>{writes++;assert.equal(value,'Kitchen');assert.equal(baseline.generation,'epoch.1');await held;});
  const second=draft.commit(async()=>{writes++;});
  release();await Promise.all([first,second]);assert.equal(writes,1,'checkmark plus blur submitted duplicate rename');
});
test('retired stream epochs cannot overwrite the current view',()=>{
  const views=new Views();const epoch=views.begin();views.snapshot(epoch,{value:'new'});views.jobs(epoch,[{id:'one',status:'succeeded'}]);
  const next=views.begin();views.snapshot(next,{value:'new session'});views.snapshot(epoch,{value:'old callback'});
  assert.deepEqual(views.value,{value:'new session'},'retired connection callback overwrote new stream state');
});

test('scene rows use ledger slot identity without an invented short address',()=>{
  const epoch='0123456789abcdef0123456789abcdef',scene={name:'Evening',slot:3,target:'Bus.Scenes.Evening',binding_revision:1};
  const initial:any={inventory:{inventory_epoch:epoch,revision:1,buses:[{bus:0,name:'Bus',enabled:true,outputs:[],scenes:[scene]}],identity_ledger:{baseline:[{target:scene.target,binding_revision:1,location:{kind:'scene',bus:0,slot:3}}],batches:[]}}};
  const found=rows(initial).find(row=>row.kind==='scene');
  assert.ok(found,'named scene missing from browser ledger rows');assert.equal(found.item.slot,3);assert.equal('short_address' in found.item,false);assert.equal(found.generation,`${epoch}.1`);
});
