import assert from 'node:assert/strict';
import {join} from 'node:path';

export async function switchingSettingsFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible}){
  await page.goto(`${origin}/buses/1`);await page.getByRole('button',{name:'Set up bus',exact:true}).click();await page.getByRole('button',{name:'Save bus',exact:true}).click();
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'relay settings did not settle');
  await idle();await page.getByRole('button',{name:'Refresh addressed',exact:true}).click();await until(async()=>!await page.getByRole('button',{name:'Refresh addressed',exact:true}).isDisabled(),'relay discovery did not settle',90000);
  const saved=await inventory(),bus=saved.buses.find(b=>b.bus===1);assert.equal(bus.outputs.length,2);
  const native=async()=>(await control({op:'snapshot'})).buses.find(b=>b.number===1).outputs;
  const before=await native(),selected=async(id='relay')=>(await native()).find(o=>o.id===id);
  const item=address=>bus.outputs.find(o=>o.short_address===address);
  const open=async(address=0)=>{const name=item(address).name,row=page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});await row.getByRole('button',{name:`Actions ${name}`,exact:true}).click();await page.getByRole('button',{name:'Fixture settings',exact:true}).click();await until(async()=>await page.getByLabel('Minimum virtual level (%)',{exact:true}).isEnabled(),'relay settings did not load');await dialogVisible();await noOverflow();};
  const save=async()=>{await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'relay settings did not confirm and close');await idle();};
  const fill=async(label,raw)=>page.getByLabel(label,{exact:true}).fill(String(raw));
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);await page.goto(`${origin}/buses/1`);await idle();await open();
    // Number then checkbox changes must retain the unblurred native edit.
    await fill('Upward on threshold',50);await page.getByLabel('Disable upward off threshold',{exact:true}).uncheck();await fill('Upward off threshold',100);
    await page.getByLabel('Disable downward on threshold',{exact:true}).uncheck();await fill('Downward on threshold',150);await fill('Downward off threshold',200);await save();
    assert.deepEqual((await selected()).switching_state.thresholds,[50,100,150,200],'relay numeric edits were lost when toggling another threshold');
    assert.equal((await selected()).level,254,'relay threshold programming operated its contact');
    await open();await page.getByText('Fixture diagnostics',{exact:true}).click();await page.getByText('Actual contact: On',{exact:true}).waitFor();
    assert.equal(await page.locator('.switching-threshold-grid').evaluate(e=>getComputedStyle(e).gridTemplateColumns.split(' ').length),viewport.width===390?1:2,'relay thresholds did not adapt to phone/desktop columns');
    assert.equal(await page.getByLabel('Downward off threshold',{exact:true}).inputValue(),'200');
    await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-relay-settings.png`),fullPage:true});
    assert.ok(await page.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'relay settings overflow horizontally');
    // A second save starts with physical254/current virtual1, so UI/backend must
    // use the freshly proved virtual floor on both sides of the readback.
    await page.getByLabel('Minimum virtual level (%)',{exact:true}).fill((100/254).toFixed(1));await save();
    await open();await page.getByLabel('Minimum virtual level (%)',{exact:true}).fill((2*100/254).toFixed(1));await page.getByLabel('Maximum virtual level (%)',{exact:true}).fill((220*100/254).toFixed(1));await save();
    assert.deepEqual([(await selected()).min_level,(await selected()).max_level],[2,220],'successive relay virtual limits did not reach hardware');
    await open();await page.getByLabel('Downward off threshold',{exact:true}).fill('');const count=commands.length;await page.getByRole('button',{name:'Save settings',exact:true}).click();
    assert.equal(commands.length,count,'invalid intermediate relay threshold submitted a job');assert.deepEqual((await selected()).switching_state.thresholds,[50,100,150,200]);
    await page.getByRole('button',{name:'Cancel',exact:true}).click();
    // Restore through the real form. Matching actual snapshots verify all
    // thresholds/limits and contact, without asserting private last-status reset.
    await open();await fill('Upward on threshold',1);await page.getByLabel('Disable upward off threshold',{exact:true}).check();await page.getByLabel('Disable downward on threshold',{exact:true}).check();await fill('Downward off threshold',0);await save();
    await open();await page.getByLabel('Minimum virtual level (%)',{exact:true}).fill('100');await page.getByLabel('Maximum virtual level (%)',{exact:true}).fill('100');await save();
    const restored=await selected();assert.deepEqual(restored.switching_state.thresholds,[1,255,255,0]);assert.deepEqual([restored.min_level,restored.max_level,restored.level],[254,254,254]);
    await open(1);for(const label of ['Upward on threshold','Upward off threshold','Downward on threshold','Downward off threshold'])assert.equal(await page.getByLabel(label,{exact:true}).isDisabled(),true,'fixed relay threshold was editable');
    await page.getByText('Fixture diagnostics',{exact:true}).click();await page.getByText('Actual contact: Off',{exact:true}).waitFor();await page.getByRole('button',{name:'Cancel',exact:true}).click();
    assert.deepEqual(await inventory(),saved,'relay settings rewrote inventory identities');
  }
  // Valid partial HTTP evidence exercises unknown-feature presentation. Hardware
  // state is unchanged; native silence/collision behavior has literal UART tests.
  const intercept=async route=>{const response=await route.fetch(),body=await response.json();if(body.result?.operation==='read_output_settings'){
    const result=body.result;result.readings.switching_features={kind:'no_response'};Object.assign(result.settings.switching,{features_raw:null,adjustable_thresholds:null,physical_selection:null,quality:'partial'});
  }await route.fulfill({response,json:body});};
  await page.route(`${origin}/api/jobs/*`,intercept);
  try{await open();await page.getByText('Threshold support unknown.',{exact:false}).waitFor();assert.equal(await page.getByLabel('Upward on threshold',{exact:true}).isDisabled(),true,'unknown relay support became writable');await page.getByRole('button',{name:'Cancel',exact:true}).click();}
  finally{await page.unroute(`${origin}/api/jobs/*`,intercept);}
  const after=await native();for(const old of before){const gear=after.find(o=>o.id===old.id);for(const field of ['short_address','groups','scenes','min_level','max_level','power_on_level','failure_level','fade','level'])assert.deepEqual(gear[field],old[field],`relay workflow altered native field ${old.id}/${field}`);assert.deepEqual(gear.switching_state.thresholds,old.switching_state.thresholds);}
  assert.deepEqual(await inventory(),saved);await page.goto(`${origin}/buses/0`);await idle();
  console.log('PASS DT7 settings: phone/desktop thresholds, native0/255, number-before-toggle retention, two virtual-limit saves, fixed/unknown read-only controls, independent native restoration and binary actual contact, unchanged saved inventory');
}
