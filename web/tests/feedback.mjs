import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function feedbackFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible}){
  const saved=await inventory(),module=saved.buses[0].inputs[0];
  const row=name=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Add unaddressed',exact:true}).isEnabled(),'feedback operation did not settle');
  const native=async()=>((await control({op:'snapshot'})).buses[0].inputs.find(i=>i.id==='buttons'));
  const before=await native();
  const open=async name=>{await row(name).getByRole('button',{name:`Actions ${name}`,exact:true}).click();await page.getByRole('button',{name:'Feedback',exact:true}).click();await until(async()=>await page.getByRole('button',{name:'Refresh feedback',exact:true}).isEnabled(),'feedback read did not settle');};
  await page.goto(`${origin}/buses/0`);await idle();
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);
    for(const scope of [{name:module.name,instance:null},{name:module.instances[0].name,instance:0},{name:module.instances[2].name,instance:2}]){
      const start=commands.length;await open(scope.name);await dialogVisible();await noOverflow();
      const toggle=page.getByRole('button',{name:'Feedback active',exact:true});assert.equal(await toggle.getAttribute('aria-pressed'),'false');
      assert.deepEqual(commands.slice(start).map(c=>c.command),['read_input_feedback'],'opening feedback sent an action');
      await toggle.click();await idle();await until(async()=>await toggle.getAttribute('aria-pressed')==='true','feedback active toggle was not verified');
      assert.equal((await native()).feedback_state.find(p=>p.instance===scope.instance).active,true,'feedback toggle did not reach its native scope');
      await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-feedback-${scope.instance??'module'}.png`),fullPage:true});
      await page.getByRole('button',{name:'Close',exact:true}).click();await open(scope.name);assert.equal(await toggle.getAttribute('aria-pressed'),'true','feedback did not reopen with actual state');
      await toggle.click();await idle();await until(async()=>await toggle.getAttribute('aria-pressed')==='false','feedback restore was not verified');
      assert.equal((await native()).feedback_state.find(p=>p.instance===scope.instance).active,false);
      await page.getByRole('button',{name:'Close',exact:true}).click();assert.deepEqual(await inventory(),saved,'feedback changed saved inventory');
    }
    const start=commands.length;await open(module.instances[3].name);await page.getByText('This input does not report supported feedback.',{exact:true}).waitFor();assert.equal(await page.getByRole('button',{name:'Feedback active',exact:true}).count(),0,'unsupported input offered an activation');
    assert.deepEqual(commands.slice(start).map(c=>c.command),['read_input_feedback']);await dialogVisible();await noOverflow();await page.getByRole('button',{name:'Close',exact:true}).click();
    // Admit a real action, then lose its browser reply after the indicator turns on.
    // Prior Off evidence must not remain actionable while the hardware is On.
    for(const scope of [{name:module.name,instance:null},{name:module.instances[0].name,instance:0}]){
      await open(scope.name);const toggle=page.getByRole('button',{name:'Feedback active',exact:true});assert.equal(await toggle.getAttribute('aria-pressed'),'false');
      let lose=true;const failed=async route=>{const call=route.request().postDataJSON();if(lose&&call.command==='set_input_feedback'){lose=false;await route.fetch();await until(async()=>(await native()).feedback_state.find(p=>p.instance===scope.instance).active,'admitted feedback did not reach hardware');await route.fulfill({status:502,contentType:'application/json',body:JSON.stringify({error:'Feedback result unavailable. Refresh to inspect again.'})});}else await route.continue();};
      await page.route('**/api/jobs',failed);const count=commands.length;await toggle.click();await idle();
      await page.getByText('Feedback is unavailable. Refresh to read it again.',{exact:true}).waitFor();assert.equal(await toggle.count(),0,'failed activation left stale feedback actionable');
      assert.equal((await native()).feedback_state.find(p=>p.instance===scope.instance).active,true);assert.deepEqual(commands.slice(count).map(c=>c.command),['set_input_feedback'],'failed activation retried without refresh');
      await dialogVisible();await noOverflow();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-feedback-unconfirmed-${scope.instance??'module'}.png`),fullPage:true});
      await page.unroute('**/api/jobs',failed);await page.getByRole('button',{name:'Refresh feedback',exact:true}).click();await idle();assert.equal(await toggle.getAttribute('aria-pressed'),'true','refresh did not recover actual active feedback');
      await toggle.click();await idle();assert.equal((await native()).feedback_state.find(p=>p.instance===scope.instance).active,false);await page.getByRole('button',{name:'Close',exact:true}).click();
    }
  }
  // Hold a real admitted entry read across synchronous dismissal/navigation.
  for(const dismissal of ['close','navigate']){
    let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
    const route=async request=>{const call=request.request().postDataJSON();if(hold&&call.command==='read_input_feedback'){hold=false;const response=await request.fetch();entered();await held;await request.fulfill({response}).catch(()=>{});}else await request.continue();};
    await page.route('**/api/jobs',route);await row(module.instances[0].name).getByRole('button',{name:`Actions ${module.instances[0].name}`,exact:true}).click();await page.getByRole('button',{name:'Feedback',exact:true}).click();await admitted;
    if(dismissal==='close')await page.getByRole('button',{name:'Close',exact:true}).click();else await page.evaluate(()=>{history.pushState(null,'','/buses/0/groups');dispatchEvent(new PopStateEvent('popstate'));});
    const start=commands.length;release();await until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'dismissed feedback did not settle');await page.unroute('**/api/jobs',route);assert.equal(await page.locator('dialog').count(),0,'retired feedback read revived its dialog');assert.equal(commands.slice(start).filter(c=>c.command==='set_input_feedback').length,0,'retired feedback read sent an action');
    await page.goto(`${origin}/buses/0`);await idle();
  }
  const after=await native();assert.deepEqual(after.feedback_state,before.feedback_state);assert.deepEqual(after.settings,before.settings);assert.deepEqual(after.values,before.values);assert.deepEqual(await inventory(),saved);
  console.log('PASS feedback: phone/desktop module and instance read/toggle/reopen/restore, native scopes, unsupported input, lost replies retire stale toggles until actual refresh, unchanged inventory/input values/settings and retired entry reads');
}
