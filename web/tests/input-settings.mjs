import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function inputSettingsFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible}){
  const saved=await inventory(),input=saved.buses[0].inputs[0],button=input.instances[0];
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
  await page.screenshot({path:join(artifacts,'phone-input-event-settings.png'),fullPage:true});
  await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'input settings save did not close');await idle();
  const native=await control({op:'snapshot'}),values=native.buses[0].inputs.find(i=>i.id==='buttons').settings[0];
  assert.deepEqual(values,{enabled:false,priority:2,scheme:2,filter:247,groups:[0,255,31]},'input settings did not persist in physical model');
  assert.deepEqual(await inventory(),saved,'event settings changed saved names, bindings or reservations');
  await page.setViewportSize({width:1440,height:1024});await open(button.name);await dialogVisible();await noOverflow();
  assert.equal(await page.getByLabel('Input enabled',{exact:true}).isChecked(),false);assert.equal(await page.getByLabel('Event priority',{exact:true}).inputValue(),'2');assert.equal(await page.getByLabel('Double press',{exact:true}).isChecked(),false);
  await page.screenshot({path:join(artifacts,'desktop-input-event-settings.png'),fullPage:true});
  // Restore through the same actual form, demonstrating independent persisted fields.
  await page.getByLabel('Input enabled',{exact:true}).check();await page.getByLabel('Event priority',{exact:true}).selectOption('4');await page.getByLabel('Double press',{exact:true}).check();await page.getByLabel('Primary group',{exact:true}).selectOption('255');await page.getByLabel('Instance group 2',{exact:true}).selectOption('255');
  await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'restored input settings did not close');await idle();
  for(const instance of input.instances.filter(i=>[2,4].includes(i.instance_type))){await open(instance.name);await page.getByLabel(instance.instance_type===2?'Position changed':'Light level changed',{exact:true}).waitFor();const count=commands.length;await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal(commands.length,count,'cancelled input settings wrote hardware');}
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
  console.log('PASS input event settings: phone/desktop real menu/read/patch/readback, enable/filter/priority/groups, unchanged identity, common sensor labels, cancelled/retired dialog');
}
