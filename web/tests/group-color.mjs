import assert from 'node:assert/strict';
import {join} from 'node:path';

export async function groupColorFlow({page,context,origin,artifacts,inventory,control,until,noOverflow,dialogVisible}){
  const native=async()=>(await control({op:'snapshot'})).buses[0].outputs;
  const original=await native();
  await control({op:'add_output',bus:0,output:{id:'color-peer',short_address:3,level:140,device_type:8,colour:{active:'temperature',temperature:{value:350,coolest:220,warmest:450,physical_coolest:150,physical_warmest:550}}}});
  await page.getByRole('button',{name:'Set up bus',exact:true}).first().click();
  await page.getByRole('button',{name:'Save bus',exact:true}).click();await page.getByRole('link',{name:'Open bus',exact:true}).first().click();
  await page.getByRole('button',{name:'Add unaddressed',exact:true}).click();
  await until(async()=>await page.getByRole('button',{name:'Add unaddressed',exact:true}).isEnabled(),'group-color discovery did not finish',90000);
  await page.getByRole('link',{name:'Groups',exact:true}).click();await page.getByLabel('New group name',{exact:true}).fill('ColorTest');await page.getByRole('button',{name:'Create group',exact:true}).click();
  const card=()=>page.getByRole('region',{name:'Group color',exact:true});
  const idle=()=>until(()=>card().getByRole('button',{name:'Inspect color support',exact:true}).isEnabled(),'group color operation did not settle');
  const inspect=async()=>{await card().getByRole('button',{name:'Inspect color support',exact:true}).click();await idle();};
  const assign=async(address,checked)=>{const saved=await inventory(),name=saved.buses[0].outputs.find(o=>o.short_address===address).name,checkbox=page.getByRole('checkbox',{name:`Assign ${name} to ColorTest`,exact:true});if(await checkbox.isChecked()!==checked)await checkbox.click();await until(async()=>await checkbox.isChecked()===checked,'native assignment did not update saved checkbox');await idle();};
  await idle();await assign(0,true);await assign(3,true);await inspect();
  const baseline=await native();
  const job=async(address,command,params)=>{
    const session=await(await context.request.get(`${origin}/api/session`)).json(),snapshot=await(await context.request.get(`${origin}/api/snapshot`)).json(),saved=snapshot.inventory.Inventory;
    const output=saved.buses[0].outputs.find(o=>o.short_address===address);if(command==='set_color_representation')params={...params,expected_revision:saved.revision};
    const admission=await context.request.post(`${origin}/api/jobs`,{headers:{Origin:origin,'X-CSRF-Token':session.csrf},data:{target:output.target,command,params,generation:`${saved.inventory_epoch}.${output.binding_revision}`}});
    assert.equal(admission.status(),202);const {id}=await admission.json();let result;
    await until(async()=>{result=await(await context.request.get(`${origin}/api/jobs/${id}`)).json();return result.status!=='running';},'fixture color restoration did not settle');assert.equal(result.status,'succeeded');
  };
  for(const address of [0,3])await job(address,'set_color_representation',{color_representation:'temperature'});
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);await inspect();
    const selector=card().getByLabel('Group color representation',{exact:true});
    assert.deepEqual(await selector.locator('option').evaluateAll(options=>options.map(option=>option.value)),['','temperature']);
    await selector.selectOption('temperature');await idle();
    await card().getByRole('button',{name:'Group color values',exact:true}).click();await dialogVisible();
    await page.getByText('Actual: Mixed',{exact:true}).waitFor();
    const temperature=page.getByLabel('Temperature (K)',{exact:true});await temperature.fill('4500');
    await page.getByRole('button',{name:'Read current values',exact:true}).click();await idle();assert.equal(await temperature.inputValue(),'4500','SSE read replaced the color draft');
    await noOverflow();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-group-color.png`),fullPage:true});
    await page.getByRole('button',{name:'Apply group color',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'group color apply did not finish');await idle();
    const actual=await native();for(const fixture of actual){const before=baseline.find(o=>o.id===fixture.id);assert.equal(fixture.level,before.level,'pure color changed brightness');if((fixture.groups&1)!==0)assert.equal(fixture.colour_state.actual.temperature.value,222);else assert.deepEqual(fixture.colour_state?.actual,before.colour_state?.actual,'group color changed a nonmember');}
    await card().getByRole('button',{name:'Group color values',exact:true}).click();await dialogVisible();assert.equal(await temperature.inputValue(),'4504.5045','UI echoed requested color instead of rounded actual');await page.getByRole('button',{name:'Close dialog',exact:true}).click();
    await assign(1,true);await inspect();await card().getByText(/no common color representation/).waitFor();assert.ok(await card().getByRole('button',{name:'Group color values',exact:true}).isDisabled());await assign(1,false);await inspect();
    await selector.selectOption('');await idle();assert.ok(await card().getByRole('button',{name:'Group color values',exact:true}).isDisabled());
    for(const address of [0,3]){const before=baseline.find(o=>o.short_address===address);await job(address,'apply_output',{values:{ColorTemperature:1_000_000/before.colour_state.actual.temperature.value}});}
    await inspect();
  }
  const restored=await native();for(const fixture of restored){const before=baseline.find(o=>o.id===fixture.id);assert.equal(fixture.level,before.level);assert.deepEqual(fixture.colour_state?.actual,before.colour_state?.actual);}
  await assign(0,false);await assign(3,false);await control({op:'remove_fixture',id:'color-peer'});
  const after=await native();for(const before of original){const fixture=after.find(o=>o.id===before.id);assert.equal(fixture.level,before.level);assert.equal(fixture.groups,before.groups);assert.deepEqual(fixture.colour_state?.actual,before.colour_state?.actual);}
  console.log('PASS group color: real phone/desktop common support selection, mixed/uniform actuals, preserved drafts, native group write, unsupported-member controls, independent fixture selection, clear selection and native restoration');
}
