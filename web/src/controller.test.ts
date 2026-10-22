import test from 'node:test';
import assert from 'node:assert/strict';
import { Controller } from './controller.ts';
import type { Snapshot } from './model.ts';

const epoch='0123456789abcdef0123456789abcdef';
test('queued inline commits preserve original item authority and prepare revisions serially',async()=>{
  const ui=new Controller(),a={target:'Bus.Outputs.A',binding_revision:1,location:{kind:'output',bus:0,short_address:0}},b={target:'Bus.Outputs.B',binding_revision:1,location:{kind:'output',bus:0,short_address:1}};
  const initial={inventory:{inventory_epoch:epoch,revision:1,buses:[{bus:0,name:'Bus',enabled:true,outputs:[{name:'A',short_address:0,target:a.target,binding_revision:1},{name:'B',short_address:1,target:b.target,binding_revision:1}]}],identity_ledger:{baseline:[a,b],batches:[]}}} as unknown as Snapshot;
  ui.client.views.value=structuredClone(initial);ui.client.views.ready=true;
  const [first,second]=ui.currentRows;const writes:any[]=[];
  ui.client.job=async(generation,target,command,params:any)=>{
    if(command!=='rename')return {};
    writes.push({generation,target,command,params});
    const inventory=structuredClone(ui.snapshot.inventory),item=inventory.buses[0].outputs.find(o=>o.target===target)!;
    const from={...(target===a.target?a:b)},to={...from,target:`Bus.Outputs.${params.name}`,binding_revision:++inventory.revision};
    inventory.identity_ledger.batches.push({revision:inventory.revision,transitions:[{from,to}]});
    item.name=params.name;item.target=to.target;item.binding_revision=to.binding_revision;
    ui.client.views.value={...ui.snapshot,inventory};return {revision:inventory.revision};
  };
  await Promise.all([ui.rename(first.key,'Kitchen',{target:a.target,generation:`${epoch}.1`}),ui.rename(second.key,'Hall',{target:b.target,generation:`${epoch}.1`})]);
  assert.deepEqual(writes.map(w=>w.params.expected_revision),[1,2],'queued explicit commits reused a prior controller revision');
  assert.deepEqual(writes.map(w=>w.generation),[`${epoch}.1`,`${epoch}.1`],'queued commit silently refreshed original item authority');
  await assert.rejects(ui.rename(first.key,'Stale',{target:a.target,generation:`${epoch}.1`}),/changed while you were editing/);
  assert.equal(writes.length,2,'stale draft was rebased and sent');ui.client.close();
});
