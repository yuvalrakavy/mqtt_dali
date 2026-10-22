import { useState } from 'preact/hooks';
import { busReady, glob, type Bus, type Row } from './model.ts';
import { continuation } from './client.ts';
import { Modal, NameField, Progress, useUi } from './ui.tsx';

export function Groups({bus}:{bus:Bus}){
  const ui=useUi(),[slot,setSlot]=useState(0),[pattern,setPattern]=useState(''),[mode,setMode]=useState('add'),[preview,setPreview]=useState<any|null>(null),[reports,setReports]=useState<any[]|null>(null),[createName,setCreateName]=useState('');
  const group=bus.groups?.find(g=>g.group_address===slot),outputs=ui.currentRows.filter(r=>r.kind==='output'&&r.bus.bus===bus.bus);
  const groupRow=ui.currentRows.find(r=>r.kind==='group'&&r.bus.bus===bus.bus&&r.item.group_address===slot);
  const ready=busReady(ui.snapshot,bus)&&ui.client.views.ready;
  function previewPattern(){const changes=outputs.map(row=>({key:row.key,generation:row.generation,name:row.item.name,before:group?.members?.includes(row.item.short_address)??false,after:mode==='add'?(group?.members?.includes(row.item.short_address)??false)||glob(pattern,row.item.name):glob(pattern,row.item.name)})).filter(row=>row.before!==row.after);
    setPreview({changes,initial:ui.snapshot,revision:ui.snapshot.inventory.revision,groupKey:groupRow?.key,slot});}
  async function applyPattern(){const captured=preview;if(!captured)return;setPreview(null);let expected=captured.revision;
    ui.cancelled=false;try{for(const change of captured.changes){
      if(ui.cancelled)break;ui.progress=`Assigning group ${captured.slot} · ${change.name}`;ui.touch();
      if(!continuation(captured.initial,ui.snapshot,expected,bus.bus)||!ui.currentRows.some(r=>r.key===captured.groupKey))throw new Error('Group configuration changed after preview. Review the pattern again.');
      const row=ui.current(change.key);if(row.generation!==change.generation)throw new Error('A selected fixture changed after preview. Review again.');
      const result=await ui.group(row,captured.slot,change.after);expected=result.revision;
    }ui.notice=ui.cancelled?'Stopped after the current assignment. Completed changes remain saved.':'Pattern assignments verified.';}finally{ui.progress='';ui.touch();}
  }
  async function verify(){const initial=ui.snapshot,list:any[]=[];
    ui.cancelled=false;try{for(const row of outputs){
      if(ui.cancelled)break;
      if(!continuation(initial,ui.snapshot,initial.inventory.revision,bus.bus)||ui.current(row.key).generation!==row.generation)throw new Error('Configuration changed during verification. Start verification again.');
      ui.progress=`Verifying groups · ${row.item.name}`;ui.touch();
      const report=await ui.readGroups(row);list.push({key:row.key,generation:report.target_generation,report,revision:initial.inventory.revision});
    }setReports(list);ui.notice=ui.cancelled?'Verification stopped. Results include completed reads only.':'Group verification finished. Review differences and unresolved fixtures.';}finally{ui.progress='';ui.touch();}
  }
  async function resolve(entry:any,action:'adopt_groups'|'repair_groups'){
    const row=ui.current(entry.key);
    if(row.generation!==entry.generation||ui.snapshot.inventory.revision!==entry.revision)throw new Error('Configuration changed since verification. Verify groups again.');
    const result=await ui.reviewGroups(row,action,entry.report.probe.mask);
    if(result.outcome==='unconfirmed')throw new Error('Hardware result remains unconfirmed. Verify again.');
    const probe=await ui.readGroups(ui.current(entry.key));
    setReports(current=>current?.map(r=>r.key===entry.key?{...r,report:probe,revision:ui.snapshot.inventory.revision}:r)??null);ui.notice=action==='adopt_groups'?'Hardware membership adopted.':'Saved membership repaired and re-read.';
  }
  return <><Progress/><div class="groups-layout"><aside class="slots"><h2>Groups</h2>{Array.from({length:16},(_,index)=>{const saved=bus.groups?.find(g=>g.group_address===index);return <button class={slot===index?'selected':''} onClick={()=>{setSlot(index);setCreateName('');}}><span>{index}</span>{saved?.name??'Unused slot'}</button>;})}</aside><section><div class="section-heading"><h2>Group {slot}</h2><button disabled={!ready||!!ui.busy} onClick={()=>void ui.run(verify)}>Verify all groups</button></div>
    {group&&groupRow?<><div class="card group-title"><NameField row={groupRow}/><span class="hint">{group.members?.length??0} saved members · slot {slot}</span></div><div class="toolbar pattern"><input aria-label="Group name pattern" value={pattern} placeholder="Name pattern · e.g. Kitchen*" onInput={e=>setPattern(e.currentTarget.value)}/><select aria-label="Pattern assignment mode" value={mode} onChange={e=>setMode(e.currentTarget.value)}><option value="add">Add matching fixtures</option><option value="replace">Replace this group's members</option></select><button disabled={!pattern||!ready||!!ui.busy} onClick={previewPattern}>Preview pattern</button></div><p class="hint">Checkboxes show saved membership. Verification compares all 16 hardware group bits.</p><div class="device-table membership-table">{outputs.map(row=><label class="membership-row" key={row.key}><input type="checkbox" aria-label={`Assign ${row.item.name} to ${group.name}`} checked={group.members?.includes(row.item.short_address)??false} disabled={!ready||!!ui.busy} onChange={e=>{const checked=e.currentTarget.checked;void ui.run(()=>ui.group(row,slot,checked).then(()=>{}));}}/><span class="address">{row.item.short_address}</span><span>{row.item.name}</span><button type="button" disabled={!ready||!!ui.busy} onClick={e=>{e.preventDefault();void ui.run(()=>ui.blink(row));}}>Blink</button></label>)}</div></>:<div class="card"><h3>Unused group slot {slot}</h3><p class="muted">Save a group name before assigning fixtures. Existing hardware membership is adopted only after review.</p><div class="toolbar"><input aria-label="New group name" placeholder={`Group${slot+1}`} value={createName} onInput={e=>setCreateName(e.currentTarget.value)}/><button class="primary" disabled={!!ui.busy} onClick={()=>void ui.run(()=>ui.createGroup(bus,slot,createName||`Group${slot+1}`))}>Create group</button></div></div>}
    {preview&&<Modal title="Review pattern assignment" close={()=>setPreview(null)}><p>{preview.changes.length} changes in group {preview.slot}. Other groups stay unchanged.</p><ul>{preview.changes.map((c:any)=><li key={c.key}>{c.after?'Add':'Remove'} {c.name}</li>)}</ul>{!preview.changes.length&&<p>No membership changes are needed.</p>}<div class="dialog-actions"><button onClick={()=>setPreview(null)}>Cancel</button><button class="primary" disabled={!preview.changes.length||!!ui.busy} onClick={()=>void ui.run(applyPattern)}>Apply reviewed changes</button></div></Modal>}
    {reports&&<Modal title="Verify group assignments" close={()=>setReports(null)}><p class="hint">Compare every saved fixture's complete hardware mask, including unnamed group slots. Missing replies stay unresolved.</p>{reports.map(entry=>{const row=ui.currentRows.find(r=>r.key===entry.key),report=entry.report,complete=report.probe?.quality==='complete'&&!report.probe?.stale,drift=complete&&report.probe.mask!==report.expected_mask;return <section class="verify-row" key={entry.key}><h3>{row?.item.name??'Removed fixture'}</h3><p>{!complete?'Unresolved hardware reply':drift?`Different · saved ${report.expected_mask.toString(16).padStart(4,'0')} · hardware ${report.probe.mask.toString(16).padStart(4,'0')}`:'Assignments match'}</p>{drift&&row&&<div class="dialog-actions"><button disabled={!!ui.busy} onClick={()=>void ui.run(()=>resolve(entry,'adopt_groups'))}>Adopt hardware assignments</button><button class="primary" disabled={!!ui.busy} onClick={()=>void ui.run(()=>resolve(entry,'repair_groups'))}>Repair to saved assignments</button></div>}</section>;})}</Modal>}
    </section></div></>;
}
