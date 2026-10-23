import test from 'node:test';
import assert from 'node:assert/strict';
import {fixtureStatus,statusFlags} from './fixture-status.ts';
const state=(raw:number)=>Object.fromEntries([['FixtureStatusQuality','current'],['FixtureStatusRaw',raw],...statusFlags.map(([key],bit)=>[key,!!(raw&(1<<bit))])]);
test('fixture fault indicator separates faults, warnings and informational flags',()=>{
  assert.equal(fixtureStatus(state(0x34)).severity,'clear');
  for(const raw of [1,2,3,255])assert.equal(fixtureStatus(state(raw)).severity,'fault');
  for(const raw of [8,64,128])assert.equal(fixtureStatus(state(raw)).severity,'warning');
  assert.deepEqual(fixtureStatus(state(0x83)).faults,['Gear failure','Lamp failure','Power failure']);
  assert.equal(fixtureStatus(state(2)).raw,2);assert.equal(fixtureStatus(state(255)).raw,255);
  assert.equal(fixtureStatus({...state(2),Available:false,Quality:'partial'}).severity,'fault');
  assert.equal(fixtureStatus(state(4)).severity,'clear','cleared failure still raises warning');
});
test('unknown, stale and malformed status never displays retained faults or a false all-clear',()=>{
  for(const value of [{},...['unobserved','stale','unavailable','invalid'].map(q=>({...state(2),FixtureStatusQuality:q})),{...state(0),FixtureStatusRaw:null},{...state(0),FixtureStatusRaw:256},{...state(0),FixtureStatusRaw:2.5},{...state(0),LampFailure:null},{...state(0),LampFailure:'false'}]){
    const s=fixtureStatus(value);assert.equal(s.severity,'unknown');assert.equal(s.raw,null);assert.deepEqual(s.faults,[]);assert.ok(s.flags.every(f=>f.value===null));
  }
});
