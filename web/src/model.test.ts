import test from 'node:test';
import assert from 'node:assert/strict';
import { glob, rowOrigins, itemGeneration, Draft, Views } from './model.ts';

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
