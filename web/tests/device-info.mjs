import assert from 'node:assert/strict';
import {join} from 'node:path';
export async function deviceInfoFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,setup=false}){
  const idle=()=>until(()=>page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'memory read did not settle');
  await page.goto(`${origin}/buses/0`);
  if(setup){await page.getByRole('button',{name:'Set up bus',exact:true}).click();await page.getByRole('button',{name:'Save bus',exact:true}).click();await idle();await page.getByRole('button',{name:'Refresh addressed',exact:true}).click();await until(()=>page.getByRole('button',{name:'Refresh addressed',exact:true}).isEnabled(),'memory discovery did not finish',90000);}
  await idle();const saved=await inventory(),bus=saved.buses[0],white=bus.outputs.find(o=>o.short_address===0),plain=bus.outputs.find(o=>o.short_address===1),input=bus.inputs.find(o=>o.short_address===0);
  assert.ok(white&&plain&&input,'memory flow requires saved physical modules');
  const native=async()=>{const value=await control({op:'snapshot'});delete value.time_ms;for(const bus of value.buses){for(const gear of bus.outputs)delete gear.dtr;for(const input of bus.inputs)for(const key of ['dtr0','dtr1','dtr2'])delete input[key];}return value;};
  const before=await native(),row=item=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${item.name}`,exact:true})}).filter({has:page.locator('.address').filter({hasText:new RegExp(`^${item.number===undefined?item.short_address:`${input.short_address} / ${item.number}`}$`)})});
  const result=()=>page.getByRole('region',{name:'Device information result',exact:true});
  const open=async item=>{await row(item).getByRole('button',{name:`Actions ${item.name}`,exact:true}).click();await dialogVisible();await page.getByRole('button',{name:'Device information',exact:true}).click();await result().waitFor();await idle();await dialogVisible();};
  const close=async()=>{await page.getByRole('button',{name:'Close dialog',exact:true}).click();await idle();};
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);await page.goto(`${origin}/buses/0`);await idle();await open(white);
    await result().getByText('FEDCBA9876543210 (64-bit)',{exact:true}).waitFor();await result().getByText('modern',{exact:true}).waitFor();
    assert.equal(await result().locator('dd').nth(2).innerText(),'6.7');assert.equal(await result().locator('dd').nth(3).innerText(),'2.3');
    await noOverflow();assert.ok(await page.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'device information overflows');
    await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-device-information.png`),fullPage:true});
    await page.getByText('Read a memory bank',{exact:true}).click();await page.getByLabel('Memory bank',{exact:true}).fill('255');await page.getByLabel('Memory byte count',{exact:true}).fill('4');
    const first=commands.length;await page.getByRole('button',{name:'Read memory',exact:true}).click();await until(async()=>await result().locator('tbody tr').count()===4,'raw memory read did not finish');await idle();
    assert.deepEqual(commands.slice(first).map(c=>c.command),['read_memory']);assert.deepEqual(await result().locator('tbody tr td:nth-child(2)').allTextContents(),['0xFF (255)','No response','0x00 (0)','0x7F (127)']);
    await noOverflow();await dialogVisible();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-memory-bytes.png`),fullPage:true});
    await page.getByLabel('First memory location',{exact:true}).fill('254');await page.getByLabel('Memory byte count',{exact:true}).fill('2');const count=commands.length;
    await page.getByRole('button',{name:'Read memory',exact:true}).click();await page.getByRole('alert').filter({hasText:'within locations'}).first().waitFor();assert.equal(commands.length,count,'invalid range reached HAT');assert.equal(await result().count(),0,'invalid new read retained prior evidence');
    await page.getByLabel('Memory bank',{exact:true}).fill('1');await page.getByLabel('First memory location',{exact:true}).fill('0');await page.getByLabel('Memory byte count',{exact:true}).fill('1');await page.getByRole('button',{name:'Read memory',exact:true}).click();await result().getByText('unavailable',{exact:true}).waitFor();await idle();await close();
    await open(plain);await result().getByText('89ABCDEF (32-bit)',{exact:true}).waitFor();await result().getByText('legacy',{exact:true}).waitFor();assert.equal(await result().locator('dd').nth(3).innerText(),'Unknown');await close();
    await open(input);await result().getByText('unknown',{exact:true}).waitFor();assert.equal(await result().locator('dd').nth(1).innerText(),'Unknown');await result().getByText('partial',{exact:true}).waitFor();await close();
    const instance=bus.inputs[0].instances[0];await row(instance).getByRole('button',{name:`Actions ${instance.name}`,exact:true}).click();assert.equal(await page.getByRole('button',{name:'Device information',exact:true}).count(),0,'instance exposes module identity');await close();
  }
  // A held real HTTP reply must lose view authority when its dialog closes.
  let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
  const intercept=async route=>{const response=await route.fetch(),body=await response.json();if(hold&&body.result?.operation==='read_identity'){hold=false;entered();await held;}await route.fulfill({response}).catch(()=>{});};
  await page.route(`${origin}/api/jobs/*`,intercept);
  await row(white).getByRole('button',{name:`Actions ${white.name}`,exact:true}).click();await page.getByRole('button',{name:'Device information',exact:true}).click();await admitted;
  await page.getByRole('button',{name:'Close dialog',exact:true}).click();release();await idle();await page.unroute(`${origin}/api/jobs/*`,intercept);await open(plain);
  assert.equal(await result().getByText('FEDCBA9876543210 (64-bit)',{exact:true}).count(),0,'retired read populated another fixture');await close();
  assert.deepEqual(await native(),before,'memory workflow changed native state other than scratch DTRs');assert.deepEqual(await inventory(),saved,'memory workflow changed saved names/bindings');
  console.log('PASS device information: phone/desktop modern/legacy/unknown identity, exact serials, raw FF/silence/zero, strict range before submission, unsupported bank, module-only menu, retired dialog, native/config preservation');
}
