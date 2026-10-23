import assert from 'node:assert/strict';

// Hold the admission response until its real job finishes. A changed hardware
// field alone does not prove that readback and physical ownership have settled.
export async function waitForAdmittedJob(route,page,origin,until){
  const response=await route.fetch(),admission=await response.json();
  assert.equal(response.status(),202,'fault injection did not admit a real job');
  assert.equal(typeof admission.id,'string');
  let job;
  await until(async()=>{
    const response=await page.context().request.get(`${origin}/api/jobs/${admission.id}`);
    assert.equal(response.status(),200);
    job=await response.json();
    return job.status!=='running';
  },'admitted job did not finish before its response was hidden');
  assert.equal(job.status,'succeeded',JSON.stringify(job.error));
  return job;
}
