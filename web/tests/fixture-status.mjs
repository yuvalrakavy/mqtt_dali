import assert from 'node:assert/strict';
import {join} from 'node:path';
export async function fixtureStatusFlow({page,context,origin,artifacts,inventory,control,until,noOverflow,dialogVisible,emulatorPage,emulatorCard}){
  const saved=await inventory(),name='KitchenSide',item=saved.buses[0].outputs.find(o=>o.name===name);
  const native=async()=>(await control({op:'snapshot'})).buses[0].outputs;
  const before=await native(),original=before.find(o=>o.short_address===item.short_address);
  const row=()=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});
  const icon=()=>row().getByRole('button',{name:`Fixture status ${name}`,exact:true});
  const idle=()=>until(()=>page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'status read did not settle');
  const read=async()=>{
    const session=await(await context.request.get(`${origin}/api/session`)).json();
    const observed=await(await context.request.get(`${origin}/api/snapshot`)).json(),inventory=observed.inventory.Inventory;
    const target=inventory.buses[0].outputs.find(o=>o.short_address===item.short_address);
    const response=await context.request.post(`${origin}/api/jobs`,{headers:{Origin:origin,'X-CSRF-Token':session.csrf},data:{target:target.target,generation:`${inventory.inventory_epoch}.${target.binding_revision}`,command:'read_output',params:{}}});
    assert.equal(response.status(),202,await response.text());const {id}=await response.json();let result;
    await until(async()=>{result=await(await context.request.get(`${origin}/api/jobs/${id}`)).json();return result.status!=='running';},'background fixture read did not finish');assert.equal(result.status,'succeeded');
  };
  const fail=async failed=>{
    if(emulatorPage){const checkbox=emulatorCard(original.id).getByLabel(`Lamp failure ${original.id}`,{exact:true});if(await checkbox.isChecked()!==failed)await checkbox.click();await until(async()=>await checkbox.isChecked()===failed,'emulator snapshot did not confirm checkbox');}
    else await control({op:'set_lamp_failure',id:original.id,failed});
    await until(async()=>!!(await native()).find(o=>o.id===original.id).lamp_failure===failed,'emulator checkbox did not reach native fixture');
  };
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);if(emulatorPage)await emulatorPage.setViewportSize(viewport);
    await page.goto(`${origin}/buses/0`);await idle();await read();
    await until(async()=>await icon().count()===0,'healthy fixture retains a fault icon');
    const input=row().getByRole('textbox');await input.fill('UnsavedStatusDraft');
    await fail(true);await read();await until(()=>icon().isVisible(),'reported lamp failure did not reach row');
    assert.equal(await input.inputValue(),'UnsavedStatusDraft','status SSE replaced dirty inline name');await input.press('Escape');
    assert.ok((await icon().getAttribute('class')).includes('fault'));assert.equal(await icon().getAttribute('title'),'Lamp failure');
    const box=await icon().boundingBox();assert.ok(box&&box.width<=30,'fault indicator expands the row');await noOverflow();
    await icon().click();await dialogVisible();
    const dialog=page.locator('dialog');await dialog.getByText('Status: current · Raw: 0x06',{exact:true}).waitFor();
    for(const label of ['Gear failure','Lamp failure','Lamp on','Limit error','Fade running','Reset state','Missing short address','Power failure'])assert.equal(await dialog.locator('dt').filter({hasText:new RegExp(`^${label}$`)}).count(),1);
    assert.equal(await dialog.locator('dd').nth(1).innerText(),'Yes');assert.equal(await dialog.locator('dd').nth(2).innerText(),'Yes');
    await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-fixture-status.png`),fullPage:true});
    await control({op:'bus_power',bus:0,powered:false});await dialog.getByText('Status: stale · Raw: unknown',{exact:true}).waitFor();
    assert.ok((await icon().getAttribute('class')).includes('unknown'),'stale retained fault presented as current');assert.equal(await dialog.locator('dd').nth(1).innerText(),'Unknown');
    await control({op:'bus_power',bus:0,powered:true});await idle();
    await fail(false);await dialog.getByRole('button',{name:'Read current values',exact:true}).click();
    await dialog.getByText('No reported faults',{exact:true}).waitFor();assert.equal(await dialog.locator('dd').nth(1).innerText(),'No');assert.equal(await icon().count(),0,'cleared fault retained icon');
    await dialogVisible();assert.ok(await dialog.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'status dialog content overflows');
    await page.getByRole('button',{name:'Close dialog',exact:true}).click();await idle();
    const after=await native();for(const gear of before){const actual=after.find(o=>o.id===gear.id);assert.equal(actual.level,gear.level,'fault simulation changed brightness');assert.equal(actual.short_address,gear.short_address);}
    assert.deepEqual(await inventory(),saved,'fault simulation changed bridge names or bindings');
  }
  await fail(!!original.lamp_failure);await page.setViewportSize({width:390,height:844});if(emulatorPage)await emulatorPage.setViewportSize({width:390,height:844});
  console.log('PASS fixture status: native emulator toggle -> bridge read -> fault indicator/modal; clear and stale evidence, dirty names, levels and saved inventory preserved at phone/desktop sizes');
}
