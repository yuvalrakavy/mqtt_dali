import test from 'node:test';
import assert from 'node:assert/strict';
import {colorActual,colorFields,colorPatch} from './group-color.ts';

test('group color preserves untouched mixed components and limits edits to common channels',()=>{
  const fields=colorFields('rgbwaf',['red','green','blue']);
  assert.deepEqual(fields,['ColorRed','ColorGreen','ColorBlue']);
  assert.deepEqual(colorPatch({ColorRed:'25',ColorGreen:'',ColorAmber:'100'},fields,'rgbwaf'),{ColorRed:0.25});
  assert.deepEqual(colorPatch({ColorX:'0.4'},colorFields('xy',null),'xy'),{ColorX:0.4});
  assert.equal(colorActual({ColorRed:null,ColorRedMixed:true},'ColorRed','rgbwaf'),'Mixed');
  assert.equal(colorActual({ColorRed:null,ColorRedMixed:null},'ColorRed','rgbwaf'),'Unknown');
  assert.equal(colorActual({ColorRed:0.5,ColorRedMixed:false},'ColorRed','rgbwaf'),'50');
  for(const value of ['NaN','Infinity','-1','101'])assert.throws(()=>colorPatch({ColorRed:value},fields,'rgbwaf'),/valid/);
  assert.throws(()=>colorPatch({},fields,'rgbwaf'),/Change at least/);
});
