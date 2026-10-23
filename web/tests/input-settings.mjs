import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function inputSettingsFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible}){
  const saved=await inventory(),input=saved.buses[0].inputs[0],button=input.instances[0];
  assert.deepEqual([...new Set(input.instances.filter(i=>[2,3,4].includes(i.instance_type)).map(i=>i.instance_type))].sort(),[2,3,4],'browser fixture must exercise all three sensor families');
  const physicalBefore=await control({op:'snapshot'}),settingsBefore=physicalBefore.buses[0].inputs.find(i=>i.id==='buttons').settings[0];
  const row=name=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Add unaddressed',exact:true}).isEnabled(),'input settings operation did not settle');
  const open=async name=>{await row(name).getByRole('button',{name:`Actions ${name}`,exact:true}).click();await page.getByRole('button',{name:'Event settings',exact:true}).click();await until(async()=>await page.getByLabel('Input enabled',{exact:true}).isEnabled(),'input settings read did not complete');};
  await page.setViewportSize({width:390,height:844});await page.goto(`${origin}/buses/0`);await idle();
  const start=commands.length;await open(button.name);await dialogVisible();await noOverflow();
  assert.deepEqual(commands.slice(start).map(c=>c.command),['read_input_settings'],'opening settings changed hardware');
  await page.getByLabel('Input enabled',{exact:true}).uncheck();
  await page.getByLabel('Event priority',{exact:true}).selectOption('2');
  await page.getByLabel('Double press',{exact:true}).uncheck();
  await page.getByLabel('Primary group',{exact:true}).selectOption('0');
  await page.getByLabel('Instance group 2',{exact:true}).selectOption('31');
  await page.getByLabel('Short press (ms)',{exact:true}).fill('1000');
  await page.getByLabel('Double press (ms)',{exact:true}).fill('400');
  await page.getByLabel('Long repeat (ms)',{exact:true}).fill('200');
  await page.getByLabel('Stuck button (s)',{exact:true}).fill('30');
  await page.screenshot({path:join(artifacts,'phone-input-event-settings.png'),fullPage:true});
  await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'input settings save did not close');await idle();
  const native=await control({op:'snapshot'}),values=native.buses[0].inputs.find(i=>i.id==='buttons').settings[0];
  assert.deepEqual(values,{...settingsBefore,enabled:false,priority:2,scheme:2,filter:247,groups:[0,255,31],button_timers:{...settingsBefore.button_timers,short:50,double:20,repeat:10,stuck:30}},'input settings did not persist or changed unrelated native settings');
  assert.deepEqual(await inventory(),saved,'event settings changed saved names, bindings or reservations');
  await page.setViewportSize({width:1440,height:1024});await open(button.name);await dialogVisible();await noOverflow();
  assert.equal(await page.getByLabel('Input enabled',{exact:true}).isChecked(),false);assert.equal(await page.getByLabel('Event priority',{exact:true}).inputValue(),'2');assert.equal(await page.getByLabel('Double press',{exact:true}).isChecked(),false);
  assert.equal(await page.getByLabel('Short press (ms)',{exact:true}).inputValue(),'1000');
  assert.equal(await page.getByLabel('Double press (ms)',{exact:true}).inputValue(),'400');
  assert.equal(await page.getByLabel('Long repeat (ms)',{exact:true}).inputValue(),'200');
  assert.equal(await page.getByLabel('Stuck button (s)',{exact:true}).inputValue(),'30');
  await page.screenshot({path:join(artifacts,'desktop-input-event-settings.png'),fullPage:true});
  // Restore through the same actual form, demonstrating independent persisted fields.
  await page.getByLabel('Input enabled',{exact:true}).check();await page.getByLabel('Event priority',{exact:true}).selectOption('4');await page.getByLabel('Double press',{exact:true}).check();await page.getByLabel('Primary group',{exact:true}).selectOption('255');await page.getByLabel('Instance group 2',{exact:true}).selectOption('255');
  await page.getByLabel('Short press (ms)',{exact:true}).fill('500');await page.getByLabel('Double press (ms)',{exact:true}).fill('0');await page.getByLabel('Long repeat (ms)',{exact:true}).fill('160');await page.getByLabel('Stuck button (s)',{exact:true}).fill('20');
  await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'restored input settings did not close');await idle();
  for(const instance of input.instances.filter(i=>[2,3,4].includes(i.instance_type))){
    const kind=instance.instance_type;
    for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
      await page.setViewportSize(viewport);await open(instance.name);await dialogVisible();await noOverflow();
      await page.getByLabel(kind===2?'Position changed':kind===3?'Occupied':'Light level changed',{exact:true}).waitFor();
      const initial=(await control({op:'snapshot'})).buses[0].inputs.find(i=>i.id==='buttons').settings[instance.number].sensor_settings;
      await page.getByLabel('Report interval (s)',{exact:true}).fill('255');await page.getByLabel('Deadtime (ms)',{exact:true}).fill('1000');
      if(kind===3)await page.getByLabel('Hold time (s)',{exact:true}).fill('1');
      if(kind===4){await page.getByLabel('Hysteresis (%)',{exact:true}).fill('25');await page.getByLabel('Minimum hysteresis (sensor units)',{exact:true}).fill('255');}
      await page.getByRole('button',{name:'Save settings',exact:true}).scrollIntoViewIfNeeded();
      await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-sensor-settings-${kind}.png`),fullPage:true});
      await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'sensor settings save did not close');await idle();
      const actual=(await control({op:'snapshot'})).buses[0].inputs.find(i=>i.id==='buttons').settings[instance.number].sensor_settings;
      assert.deepEqual(actual,{...initial,report:255,deadtime:20,...(kind===3?{hold:0}:{}),...(kind===4?{hysteresis:25,hysteresis_min:255}:{})},'sensor native values or exact units did not persist');
      await open(instance.name);assert.equal(await page.getByLabel('Report interval (s)',{exact:true}).inputValue(),'255');assert.equal(await page.getByLabel('Deadtime (ms)',{exact:true}).inputValue(),'1000');
      if(kind===3)assert.equal(await page.getByLabel('Hold time (s)',{exact:true}).inputValue(),'1');
      if(kind===4){assert.equal(await page.getByLabel('Hysteresis (%)',{exact:true}).inputValue(),'25');assert.equal(await page.getByLabel('Minimum hysteresis (sensor units)',{exact:true}).inputValue(),'255');}
      await page.getByLabel('Report interval (s)',{exact:true}).fill(String(initial.report));await page.getByLabel('Deadtime (ms)',{exact:true}).fill(String(initial.deadtime*50));
      if(kind===3)await page.getByLabel('Hold time (s)',{exact:true}).fill(String(initial.hold===0?1:initial.hold*10));
      if(kind===4){await page.getByLabel('Hysteresis (%)',{exact:true}).fill(String(initial.hysteresis));await page.getByLabel('Minimum hysteresis (sensor units)',{exact:true}).fill(String(initial.hysteresis_min));}
      await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'restored sensor settings did not close');await idle();
      assert.deepEqual(await inventory(),saved,'sensor settings changed saved inventory');
    }
  }
  // A held entry read must not revive the form after actual dismissal/navigation.
  for(const dismissal of ['cancel','navigate']){
    let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
    await page.route(`${origin}/api/jobs`,async route=>{if(hold&&route.request().postDataJSON().command==='read_input_settings'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
    try{
      await row(button.name).getByRole('button',{name:`Actions ${button.name}`,exact:true}).click();await page.getByRole('button',{name:'Event settings',exact:true}).click();await admitted;
      if(dismissal==='cancel')await page.getByRole('button',{name:'Cancel',exact:true}).click();else await page.evaluate(()=>{history.pushState(null,'','/buses/0/groups');dispatchEvent(new PopStateEvent('popstate'));});
      const stopped=commands.length;release();await until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'dismissed input settings did not settle');assert.equal(commands.length,stopped,'input settings continued after dismissal');assert.equal(await page.locator('dialog').count(),0);
    }finally{release?.();await page.unroute(`${origin}/api/jobs`);}
    await page.goto(`${origin}/buses/0`);await idle();
  }
  assert.deepEqual(await inventory(),saved);await noOverflow();
  console.log('PASS input event settings: phone/desktop real read/patch/reopen/restore, common settings/button timers/all sensor families with exact units and one-second hold, unchanged identity, cancelled/retired dialog');
}
