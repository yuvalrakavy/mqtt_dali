import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

export async function scenesFlow({page,context,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,closeDialog}){
  const row=name=>page.locator(`[data-scene-fixture="${name}"]`);
  const saved=async slot=>(await inventory()).buses[0].scenes?.find(s=>s.slot===slot);
  const native=async()=>(await control({op:'snapshot'})).buses[0].outputs;
  const idle=async()=>until(async()=>!await page.getByRole('button',{name:'Verify scene',exact:true}).isDisabled(),'scene operation did not settle');
  const named=name=>page.getByRole('textbox',{name:`Name ${name}`,exact:true});
  const choose=async slot=>{await page.getByLabel('Scene slot',{exact:true}).selectOption(String(slot));};
  async function create(name){await page.getByLabel('New scene name',{exact:true}).fill(name);await page.getByRole('button',{name:'Create scene',exact:true}).click();await named(name).waitFor();await idle();}
  async function apiJob(target,command,params){
    const session=await (await context.request.get(`${origin}/api/session`)).json();
    const snapshot=await (await context.request.get(`${origin}/api/snapshot`)).json();
    const output=snapshot.inventory.Inventory.buses[0].outputs.find(o=>o.target===target);
    const reply=await context.request.post(`${origin}/api/jobs`,{headers:{Origin:origin,'X-CSRF-Token':session.csrf},data:{target,command,params,generation:`${snapshot.inventory.Inventory.inventory_epoch}.${output.binding_revision}`}});
    assert.equal(reply.status(),202);const admission=await reply.json();let result;
    await until(async()=>{result=await(await context.request.get(`${origin}/api/jobs/${admission.id}`)).json();return result.status!=='running';},'external operator command did not finish');
    assert.equal(result.status,'succeeded');return result.result;
  }
  await page.getByRole('link',{name:'Scenes',exact:true}).click();await page.getByRole('heading',{name:'Scene 0',exact:true}).waitFor();
  assert.equal((await inventory()).buses[0].scenes?.length??0,0,'browsing created a scene');await create('Evening');
  await named('Evening').fill('Relax');await named('Evening').press('Enter');await named('Relax').waitFor();await idle();
  const before=await native();
  await page.getByLabel('Scene action KitchenMain',{exact:true}).selectOption('brightness');
  await page.getByLabel('Brightness (%) KitchenMain',{exact:true}).fill('40');
  await page.getByLabel('Scene colour KitchenMain',{exact:true}).selectOption('temperature');
  await page.getByLabel('Temperature (K) KitchenMain',{exact:true}).fill('3500');
  await page.getByRole('button',{name:'Save scene settings KitchenMain',exact:true}).click();
  await until(async()=>(await saved(0))?.contents?.some(r=>r.setting.color.kind==='temperature'&&r.setting.color.value===286),'inline scene colour was not saved');await idle();
  assert.equal((await native())[0].scenes[0],102,'scene brightness did not reach native memory');assert.equal((await native())[0].level,before[0].level,'scene edit activated brightness');
  await writeFile(join(artifacts,'fail-next-save'),'1');
  await page.getByLabel('Scene action KitchenSide',{exact:true}).selectOption('brightness');await page.getByLabel('Brightness (%) KitchenSide',{exact:true}).fill('20');
  await page.getByRole('button',{name:'Save scene settings KitchenSide',exact:true}).click();await row('KitchenSide').getByRole('alert').waitFor();
  assert.equal(await page.getByLabel('Brightness (%) KitchenSide',{exact:true}).inputValue(),'20','failed save lost scene draft');
  await page.getByRole('button',{name:'Save scene settings KitchenSide',exact:true}).click();await until(async()=>(await native())[1].scenes[0]===51,'scene save retry did not program');await idle();
  assert.equal((await native())[1].level,before[1].level,'scene retry activated brightness');
  await page.getByLabel('Scene action HallNew',{exact:true}).selectOption('keep');await page.getByLabel('Scene colour HallNew',{exact:true}).selectOption('xy');await page.getByLabel('x HallNew',{exact:true}).fill('0.2');
  await page.getByRole('button',{name:'Save scene settings HallNew',exact:true}).click();await until(async()=>(await saved(0))?.contents?.some(r=>r.setting.color.kind==='xy'&&r.setting.color.x===13107&&r.setting.color.y===null),'xy attribute MASK did not persist');await idle();
  await choose(2);await create('Colour');await page.getByLabel('Scene action HallNew',{exact:true}).selectOption('brightness');await page.getByLabel('Brightness (%) HallNew',{exact:true}).fill('70');await page.getByLabel('Scene colour HallNew',{exact:true}).selectOption('rgbwaf');
  await page.getByLabel('Red (%) HallNew',{exact:true}).fill('40');await page.getByLabel('Blue (%) HallNew',{exact:true}).fill('20');await page.getByLabel('White (%) HallNew',{exact:true}).fill('10');await page.getByLabel('RGBWAF control HallNew',{exact:true}).selectOption('143');
  await page.getByRole('button',{name:'Save scene settings HallNew',exact:true}).click();await until(async()=>(await native())[2].scenes[2]===178,'RGBWAF scene did not program');await idle();
  const rgb=(await saved(2)).contents[0].setting.color;assert.deepEqual(rgb,{kind:'rgbwaf',channels:[102,null,51,25,null,null],control:143},'RGBWAF values or attribute MASK changed');assert.deepEqual((await native())[2].colour_state.actual,before[2].colour_state.actual,'stored colour activated live output');
  await choose(0);await named('Relax').waitFor();await idle();
  await page.getByLabel('Scene fixture pattern',{exact:true}).fill('KitchenMain');await page.getByRole('button',{name:'Capture current',exact:true}).click();await page.getByRole('heading',{name:'Review captured output',exact:true}).waitFor();await dialogVisible();
  const captures=commands.filter(c=>c.command==='capture_scene_row').length;
  const target='Bus1.Outputs.KitchenMain';
  await apiJob(target,'apply_output',{values:{PowerState:false}});
  await page.getByRole('button',{name:'Apply reviewed settings',exact:true}).click();await until(async()=>(await native())[0].scenes[0]===before[0].level,'capture did not apply frozen candidate');await idle();
  assert.equal(commands.filter(c=>c.command==='capture_scene_row').length,captures,'capture was repeated after approval');assert.equal((await native())[0].level,0,'capture save changed actual output');
  await page.getByLabel('Scene fixture pattern',{exact:true}).fill('Kitchen*');await page.getByRole('button',{name:'Clear selected',exact:true}).click();await dialogVisible();assert.equal(await page.locator('dialog .verify-row').count(),2);await closeDialog();assert.equal((await native())[0].scenes[0],before[0].level,'cancelled clear changed native memory');
  await choose(1);await create('Dinner');await page.getByLabel('Scene fixture pattern',{exact:true}).fill('KitchenMain');await page.getByLabel('Copy source scene',{exact:true}).selectOption('0');await page.getByRole('button',{name:'Copy saved',exact:true}).click();await page.getByRole('heading',{name:'Copy saved scene 0 → 1',exact:true}).waitFor();await dialogVisible();await page.getByRole('button',{name:'Apply reviewed settings',exact:true}).click();await until(async()=>(await native())[0].scenes[1]===before[0].level,'saved scene copy did not reach native destination');await idle();
  assert.equal((await native())[1].scenes[1],42,'scoped copy changed unselected native fixture');assert.equal((await native())[0].scenes[0],before[0].level,'copy changed source scene');
  await page.getByLabel('Scene fixture pattern',{exact:true}).fill('KitchenSide');await page.getByRole('button',{name:'Verify scene',exact:true}).click();await page.getByRole('heading',{name:'Verify native scenes',exact:true}).waitFor();await dialogVisible();
  await page.getByRole('button',{name:'Review adoption',exact:true}).click();await page.getByRole('heading',{name:'Adopt reviewed native settings',exact:true}).waitFor();await page.getByRole('button',{name:'Apply reviewed settings',exact:true}).click();
  await until(async()=>(await saved(1)).contents?.some(r=>r.setting.level.value===42),'reviewed adoption did not persist native setting');await idle();
  await page.getByRole('button',{name:'Verify scene',exact:true}).click();await page.getByRole('heading',{name:'Verify native scenes',exact:true}).waitFor();await page.getByRole('button',{name:'Review repair',exact:true}).click();await page.getByRole('heading',{name:'Repair saved settings',exact:true}).waitFor();await dialogVisible();assert.equal(await page.locator('dialog .verify-row').count(),0,'matching settings offered unnecessary repair');assert.ok(await page.getByRole('button',{name:'Apply reviewed settings',exact:true}).isDisabled());await closeDialog();
  await page.getByRole('button',{name:'Clear selected',exact:true}).click();await page.getByRole('button',{name:'Apply reviewed settings',exact:true}).click();await until(async()=>(await native())[1].scenes[1]===255,'reviewed clear did not remove participation');await idle();assert.equal((await native())[0].scenes[1],before[0].level,'clear changed an unselected fixture');
  await page.getByLabel('Recall scope',{exact:true}).selectOption('0');await page.getByRole('button',{name:'Recall',exact:true}).click();await until(async()=>(await native())[0].level===before[0].level,'group scene recall did not change selected fixture');await idle();assert.equal((await native())[1].level,before[1].level,'ignored fixture changed on recall');
  await page.getByLabel('Scene fixture pattern',{exact:true}).fill('');await page.getByRole('button',{name:'Verify all 16',exact:true}).click();await page.getByRole('heading',{name:'Verify native scenes',exact:true}).waitFor({timeout:90000});await dialogVisible();assert.equal(await page.locator('dialog .verify-row').count(),48,'verify all skipped unnamed slots');await closeDialog();
  const publicSnapshot=await(await context.request.get(`${origin}/api/snapshot`)).json();assert.ok(publicSnapshot.inventory.Inventory.buses[0].scenes.every(s=>!('contents' in s)),'public snapshot exposed saved native contents');
  await page.screenshot({path:join(artifacts,'phone-scenes.png'),fullPage:true});await noOverflow();
  await page.setViewportSize({width:1440,height:1024});await page.evaluate(()=>new Promise(r=>{scrollTo(0,0);requestAnimationFrame(()=>requestAnimationFrame(r));}));await page.screenshot({path:join(artifacts,'desktop-scenes.png'),fullPage:false});await noOverflow();
  await page.getByRole('button',{name:'Scene actions',exact:true}).click();await page.getByLabel('Scene description',{exact:true}).fill('Dining native memory');await page.getByRole('button',{name:'Save description',exact:true}).click();await until(async()=>(await saved(1)).description==='Dining native memory','scene description did not persist');await idle();
  await page.getByRole('button',{name:'Scene actions',exact:true}).click();await page.getByRole('button',{name:'Remove scene name',exact:true}).click();await page.getByRole('button',{name:'Remove scene',exact:true}).click();await until(async()=>!(await saved(1)),'scene removal did not persist');assert.equal((await native())[0].scenes[1],before[0].level,'removing scene name erased native scene');
  await page.setViewportSize({width:390,height:844});await choose(0);await named('Relax').waitFor();await idle();
  // A completed creation on the previous slot cannot erase the next slot's name draft.
  await choose(3);await page.getByLabel('New scene name',{exact:true}).fill('Detached');
  let created,releaseCreate;const creation=new Promise(r=>created=r),heldCreate=new Promise(r=>releaseCreate=r);let holdCreate=true;
  await page.route(`${origin}/api/jobs`,async route=>{if(holdCreate&&route.request().postDataJSON().command==='create_scene'){holdCreate=false;const response=await route.fetch();created();await heldCreate;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
  try{await page.getByRole('button',{name:'Create scene',exact:true}).click();await creation;await choose(4);await page.getByLabel('New scene name',{exact:true}).fill('KeepThisDraft');releaseCreate();await idle();assert.equal(await page.getByLabel('New scene name',{exact:true}).inputValue(),'KeepThisDraft','late creation erased the next scene name draft');}finally{releaseCreate?.();await page.unroute(`${origin}/api/jobs`);}
  await choose(0);await named('Relax').waitFor();await idle();
  let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
  await page.route(`${origin}/api/jobs`,async route=>{if(hold&&route.request().postDataJSON().command==='capture_scene_row'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
  try{await page.getByRole('button',{name:'Capture current',exact:true}).click();await admitted;const count=commands.length;await page.getByRole('link',{name:'Devices',exact:true}).click();release();await until(async()=>await page.getByRole('button',{name:'Refresh addressed',exact:true}).isEnabled(),'leaving scene capture did not release work');await new Promise(r=>setTimeout(r,100));assert.equal(commands.length,count,'scene capture continued after page navigation');}finally{release?.();await page.unroute(`${origin}/api/jobs`);}
  console.log('PASS scenes: inline Tc/xy/RGBWAF and MASK, failed save/retry, frozen capture, scoped copy/clear/adoption, matching repair review, group recall, 16-slot verify, privacy, metadata, navigation cancellation');
}
