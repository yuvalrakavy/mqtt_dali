import test from 'node:test';
import assert from 'node:assert/strict';
import { Views } from './model.ts';
import { Client, continuation } from './client.ts';
import { decodeSnapshot } from './client.ts';
test('browser unwraps the actual published Inventory envelope',()=>{
  const inventory={inventory_epoch:'0123456789abcdef0123456789abcdef',revision:1,name:'Controller',buses:[],identity_ledger:{baseline:[],batches:[]}};
  let decoded:any;
  assert.doesNotThrow(()=>{decoded=decodeSnapshot({identity:{controller:'Controller'},inventory:{Inventory:inventory},states:[]});},'published Inventory envelope was not decoded');
  assert.deepEqual(decoded.inventory.buses,[],'published Inventory envelope was not decoded');
  assert.equal(decoded.inventory.revision,1);
});
test('closing the client ends an in-flight revision waiter',async()=>{
  const client=new Client(),waiting=client.atRevision(2);client.close();
  await assert.rejects(waiting,/Session ended/);
  assert.equal(client.snapshot,null);
});
test('a batch continues only through its own confirmed revision and original bus',()=>{
  const initial={inventory:{inventory_epoch:'epoch',revision:3,buses:[{bus:0,name:'Bus1',enabled:true}]}};
  const good={inventory:{...initial.inventory,revision:4}};
  assert.equal(continuation(initial as any,good as any,4,0),true);
  assert.equal(continuation(initial as any,{inventory:{...good.inventory,revision:5}} as any,4,0),false,'batch silently rebased across external metadata');
  assert.equal(continuation(initial as any,{inventory:{...good.inventory,buses:[{bus:0,name:'Other',enabled:true}]}} as any,4,0),false,'batch followed a renamed bus');
});
class Source extends EventTarget {
  static sources:Source[]=[];onerror:(()=>void)|null=null;constructor(_url:string){super();Source.sources.push(this);}close(){}
  emit(name:string,value:any){if(name==='snapshot')value={inventory:{Inventory:{inventory_epoch:'0123456789abcdef0123456789abcdef',revision:1,buses:[],identity_ledger:{baseline:[],batches:[]}}},...value};this.dispatchEvent(new MessageEvent(name,{data:JSON.stringify(value)}));}
}
test('actual client keeps newer SSE views when completion precedes delayed POST admission and ignores old sources',async()=>{
  const priorFetch=globalThis.fetch,priorSource=globalThis.EventSource;
  let release!:(value:Response)=>void;const pending=new Promise<Response>(resolve=>release=resolve);const paths:string[]=[];
  globalThis.EventSource=Source as any;
  globalThis.fetch=async(input:any)=>{paths.push(String(input));if(input==='/api/session')return Response.json({csrf:'secret'});if(input==='/api/jobs')return pending;throw new Error(`Unexpected HTTP bootstrap/read: ${input}`);};
  const client=new Client();
  try {
    await client.start();const first=Source.sources.at(-1)!;
    first.emit('snapshot',{states:[{PowerState:true}]});
    const result=client.job('epoch.1','Bus.Outputs.Light','apply_output',{values:{PowerState:false}});
    first.emit('snapshot',{states:[{PowerState:false}]});
    first.emit('jobs',{jobs:[{id:'job',status:'succeeded',result:{actual:false}}]});
    release(Response.json({id:'job',status:'running'}));assert.deepEqual(await result,{actual:false});
    assert.deepEqual(client.snapshot?.states,[{PowerState:false}],'HTTP admission overwrote newer actual state');
    assert.equal(client.jobs[0].status,'succeeded','late 202 downgraded completed stream job');
    await client.start();const second=Source.sources.at(-1)!;second.emit('snapshot',{states:[{PowerState:false}],source:'new'});
    first.emit('snapshot',{states:[],source:'old'});assert.equal((client.snapshot as any).source,'new','old connection callback replaced the live view');
    first.emit('input',{target:'retired',action:'pressed'});assert.equal(client.events.length,0,'retired stream delivered an old input action');
    first.onerror?.();assert.equal(client.views.ready,true,'retired stream error invalidated the new connection');
    assert.ok(!paths.includes('/api/snapshot'),'browser bootstrapped an unordered HTTP snapshot');
    second.onerror?.();second.emit('snapshot',{source:'after error'});assert.equal(client.views.ready,false,'failed stream callback re-enabled stale controls');
  } finally {client.close();globalThis.fetch=priorFetch;globalThis.EventSource=priorSource;}
});
test('a delayed GET job result cannot replace newer stream observations or a terminal job',async()=>{
  const priorFetch=globalThis.fetch,priorSource=globalThis.EventSource;
  let release!:(value:Response)=>void,entered!:()=>void;
  const held=new Promise<Response>(r=>release=r),inside=new Promise<void>(r=>entered=r);
  globalThis.EventSource=Source as any;globalThis.fetch=async(input:any)=>{
    if(input==='/api/session')return Response.json({csrf:'secret'});
    if(input==='/api/jobs')return Response.json({id:'slow',status:'running'});
    if(input==='/api/jobs/slow'){entered();return held;}
    throw new Error(`Unexpected request ${input}`);
  };
  const client=new Client();try{
    await client.start();const source=Source.sources.at(-1)!;source.emit('snapshot',{states:[{PowerState:true}]});
    const result=client.job('epoch.1','Light','apply_output',{});void result.catch(()=>{});await inside;
    source.emit('snapshot',{states:[{PowerState:false}]});source.emit('jobs',{jobs:[{id:'slow',status:'succeeded',result:{actual:false}}]});
    release(Response.json({id:'slow',status:'running'}));await new Promise(setImmediate);
    assert.equal(client.jobs[0].status,'succeeded','late job GET downgraded the terminal stream book');
    assert.deepEqual(client.snapshot?.states,[{PowerState:false}],'late job GET replaced actual observations');
    assert.deepEqual(await result,{actual:false});
  }finally{client.close();globalThis.fetch=priorFetch;globalThis.EventSource=priorSource;}
});
test('a session reply arriving after close cannot restore CSRF or reconnect',async()=>{
  const priorFetch=globalThis.fetch;let release!:(value:Response)=>void;const held=new Promise<Response>(r=>release=r);
  globalThis.fetch=async()=>held;const client=new Client();try{
    const start=client.start();client.close();release(Response.json({csrf:'retired-secret'}));await assert.rejects(start,/Session ended/);
    assert.equal(client.csrf,'','retired session reply restored authentication');assert.equal(client.views.ready,false);
  }finally{client.close();globalThis.fetch=priorFetch;}
});

test('a completed summary fetches its full result without changing streamed views',async()=>{
  const priorFetch=globalThis.fetch;const client=new Client();let gets=0;
  client.views.ready=true;client.views.value={marker:'stream'};
  client.views.jobBook=[{id:'summary',status:'succeeded',result:null}];
  globalThis.fetch=async(input:any)=>{if(input==='/api/jobs')return Response.json({id:'summary'});if(input==='/api/jobs/summary'){gets++;return Response.json({id:'summary',status:'succeeded',result:{fixtures:[{saved:42}]}});}throw new Error(`Unexpected request ${input}`);};
  try{assert.deepEqual(await client.job('epoch.1','Scene','read_scene_settings'),{fixtures:[{saved:42}]},'summary was mistaken for a complete command result');assert.equal(gets,1);assert.equal((client.snapshot as any).marker,'stream');assert.equal(client.jobs[0].result,null);}finally{client.close();globalThis.fetch=priorFetch;}
});
