import { useEffect, useRef, useState } from 'preact/hooks';
import { busReady, glob, itemGeneration, values, type Bus, type PhysicalRow, type SceneRow } from './model.ts';
import { SceneScope, applyChanges, freezeChanges, type Change } from './scene-controller.ts';
import { SettingDraft, channels, fromStored, ignored, summary, validSetting, type Setting } from './scene-settings.ts';
import { Modal, NameField, Progress, useUi } from './ui.tsx';
import './scenes.css';

interface PrivateView {revision:number;epoch:number;key?:string;rows:Map<string,Setting|null>}
interface Preview {title:string;scope:SceneScope;changes:Change[];unresolved:string[]}
interface Evidence {scope:SceneScope;row:PhysicalRow;slot:number;report:any;candidate:Setting|null}

function SceneFixture({row,visible,view,makeScope}:{row:PhysicalRow;visible:boolean;view:PrivateView|null;makeScope:()=>SceneScope}){
  const ui=useUi(),[draft,setDraft]=useState<SettingDraft|null>(null),[scope,setScope]=useState<SceneScope|null>(null),[seen,setSeen]=useState(-1),[expanded,setExpanded]=useState(false);
  const current=view&&view.revision===ui.snapshot.inventory.revision&&view.epoch===ui.client.views.epoch&&view.rows.has(row.key);
  const shownSaved=current&&view?view.rows.get(row.key)!:draft?.original??null;
  const unconfirmed=draft?.error.startsWith('Saved settings are not confirmed');
  useEffect(()=>()=>{if(scope)scope.cancelled=true;},[scope]);
  useEffect(()=>{
    if(current&&view&&(!draft||!draft.dirty&&!draft.pending)&&(seen!==view.revision||!draft)){
      setDraft(new SettingDraft(view.rows.get(row.key)!));setScope(null);setSeen(view.revision);
    }
  },[view,draft?.dirty,draft?.pending]);
  const ready=busReady(ui.snapshot,row.bus)&&ui.client.views.ready;
  function editing(){if(!draft?.dirty){setScope(makeScope());} }
  const conflict=!!draft?.dirty&&(!scope||scope.revision!==ui.snapshot.inventory.revision||scope.epoch!==ui.client.views.epoch);
  const active=ui.snapshot.identification,blinking=active?.target===row.item.target&&['starting','blinking','restoring'].includes(active.phase);
  const caps=values(ui.snapshot,row).ColorCapabilities as {supported_representations?:string[]|null;rgbwaf_assignments?:string[]|null}|undefined;
  function commit(){
    if(!draft||!scope||!draft.dirty)return;
    void draft.commit(async setting=>{
      if(ui.busy)throw new Error('Wait for the current operation, then save this draft.');
      ui.busy++;ui.error='';ui.touch();
      try{ui.cancelled=false;await scope.mutate(row,'set_scene_row',{setting});ui.notice=`${row.item.name}: saved settings verified.`;}
      finally{ui.busy--;ui.touch();}
    }).catch(()=>{}).finally(()=>ui.touch());ui.touch();
  }
  const disabled=!ready||!current||!view?.key||!!ui.busy||!!draft?.pending||conflict;
  const field=(name:string,label:string)=><label class="scene-field" key={name}>{label}<input type="number" step="any" aria-label={`${label} ${row.item.name}`} value={draft?.text(name)??''} disabled={disabled} placeholder="MASK" onInput={e=>{editing();draft!.change(name,e.currentTarget.value);ui.touch();}}/></label>;
  return <article hidden={!visible&&!draft?.dirty&&!draft?.error} class={`scene-fixture ${blinking?'identifying':''}`} data-scene-fixture={row.item.name} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget as Node|null))commit();}} onKeyDown={e=>{if(e.key==='Enter'&&(e.target as HTMLElement).tagName!=='BUTTON'){e.preventDefault();commit();}}}>
    <div class="scene-fixture-title"><span class="address">{row.item.short_address}</span><strong>{row.item.name}</strong><span class={`pill ${current&&shownSaved&&!unconfirmed?'':'unknown'}`}>{unconfirmed?'Saved · unconfirmed':current?shownSaved?'Saved':'Unmanaged':'Refreshing'}</span><button disabled={!ready||!!ui.busy||active?.phase==='restoring'} aria-label={`${blinking?'Stop blink':'Blink'} ${row.item.name}`} onClick={()=>void ui.run(()=>ui.blink(row))}>{blinking?'Stop':'Blink'}</button></div>
    {draft?<><div class="scene-editor"><label class="scene-field">Action<select aria-label={`Scene action ${row.item.name}`} value={draft.action} disabled={disabled} onChange={e=>{editing();draft.chooseAction(e.currentTarget.value as SettingDraft['action']);ui.touch();}}><option value="ignore">Leave unchanged</option><option value="off">Off</option><option value="brightness">Brightness</option><option value="keep">Keep brightness</option></select></label>
      {draft.action==='brightness'&&field('brightness','Brightness (%)')}
      {draft.action!=='ignore'&&<label class="scene-field">Color<select aria-label={`Scene color ${row.item.name}`} value={draft.family} disabled={disabled} onChange={e=>{editing();draft.chooseFamily(e.currentTarget.value as Setting['color']['kind']);ui.touch();}}><option value="mask">Keep color</option>{[['temperature','White temperature'],['xy','CIE xy'],['rgbwaf','RGBWAF']].map(([value,label])=><option value={value} disabled={!!caps?.supported_representations&&!caps.supported_representations.includes(value)&&draft.family!==value}>{label}</option>)}</select></label>}
      {draft.action!=='ignore'&&draft.family==='temperature'&&field('temperature','Temperature (K)')}
      {draft.action!=='ignore'&&draft.family==='xy'&&<>{field('x','x')}{field('y','y')}</>}
      {draft.action!=='ignore'&&draft.family==='rgbwaf'&&<>{channels.map(name=>field(name,`${name[0].toUpperCase()+name.slice(1)} (%)`))}<label class="scene-field">RGBWAF control<select aria-label={`RGBWAF control ${row.item.name}`} value={draft.text('control')} disabled={disabled} onChange={e=>{editing();draft.change('control',e.currentTarget.value);ui.touch();}}><option value="">Keep control (MASK)</option>{Array.from({length:64},(_,i)=><option value={128+i}>Normalized · {i.toString(2).padStart(6,'0')}</option>)}{draft.text('control')!==''&&Number(draft.text('control'))<128&&<option value={draft.text('control')} disabled>Unsupported native mode {draft.text('control')}</option>}</select></label></>}
      {draft.dirty&&<button class="primary scene-save" aria-label={`Save scene settings ${row.item.name}`} disabled={disabled} onPointerDown={e=>e.preventDefault()} onClick={commit}>{draft.pending?'Saving…':'✓'}</button>}
    </div>{(conflict||draft.error)&&<p class="error" role="alert">{draft.error||'Configuration changed while this row was being edited. Review it again.'} <button class="text-button" disabled={!!draft.pending} onClick={()=>{setDraft(null);setSeen(-1);}}>Discard draft</button></p>}
    <div class="scene-row-foot"><span class="hint">{summary(shownSaved)}{draft.dirty?' · Unsaved draft':''}</span><button class="text-button" onClick={()=>setExpanded(!expanded)}>{expanded?'Hide native values':'Native values'}</button></div>
    {expanded&&<div class="scene-native"><p class="hint">Saved native codes; blank attributes are MASK. Color support: {caps?.supported_representations?.join(', ')||'unconfirmed'}{caps?.rgbwaf_assignments?` · Assigned: ${caps.rgbwaf_assignments.join(', ')}`:''}.</p><pre>{JSON.stringify(shownSaved,null,2)}</pre><button disabled={!ready||!!ui.busy} onClick={()=>void ui.run(()=>ui.read(row))}>Read capabilities</button></div>}</>:<p class="hint">Loading saved settings…</p>}
  </article>;
}

export function Scenes({bus}:{bus:Bus}){
  const ui=useUi(),[slot,setSlot]=useState(0),[pattern,setPattern]=useState(''),[createName,setCreateName]=useState(''),[view,setView]=useState<PrivateView|null>(null),[loadError,setLoadError]=useState(''),[reload,setReload]=useState(0),[preview,setPreview]=useState<Preview|null>(null),[reports,setReports]=useState<Evidence[]|null>(null),[copySlot,setCopySlot]=useState(1),[group,setGroup]=useState('bus'),[more,setMore]=useState<SceneRow|null>(null),[description,setDescription]=useState(''),[remove,setRemove]=useState<SceneRow|null>(null);
  const scopes=useRef(new Set<SceneScope>()),revision=ui.snapshot.inventory.revision,epoch=ui.client.views.epoch;
  const scene=ui.currentRows.find((r):r is SceneRow=>r.kind==='scene'&&r.bus.bus===bus.bus&&r.item.slot===slot);
  const outputs=ui.currentRows.filter((r):r is PhysicalRow=>r.kind==='output'&&r.bus.bus===bus.bus),matched=outputs.filter(r=>glob(pattern,r.item.name));
  const ready=busReady(ui.snapshot,bus)&&ui.client.views.ready,loaded=!!view&&view.revision===revision&&view.epoch===epoch&&view.key===scene?.key;
  function scope(){for(const old of scopes.current)if(old.cancelled)scopes.current.delete(old);const s=new SceneScope(ui,bus.bus,slot);scopes.current.add(s);return s;}
  useEffect(()=>()=>{for(const s of scopes.current)s.cancelled=true;scopes.current.clear();},[slot]);
  useEffect(()=>{
    const s=scope();let active=true;setLoadError('');setView(null);
    if(ui.client.views.ready)void s.saved().then(rows=>{if(active)setView({revision:s.revision,epoch:s.epoch,key:s.scene?.key,rows});}).catch(error=>{if(active)setLoadError(error.message);});
    return()=>{active=false;s.cancelled=true;scopes.current.delete(s);};
  },[slot,revision,epoch,ui.client.views.ready,reload]);
  function select(value:number){if(ui.busy){ui.cancelled=true;for(const s of scopes.current)s.cancelled=true;}setSlot(value);setCreateName('');setPreview(null);setReports(null);}
  function closeReports(){for(const entry of reports??[])entry.scope.cancelled=true;setReports(null);}
  function before(row:PhysicalRow){if(!loaded||!view?.rows.has(row.key))throw new Error('Wait for complete saved settings.');return view.rows.get(row.key)!;}
  function plan(title:string,s:SceneScope,changes:Change[],unresolved:string[]=[]){s.assert();setPreview({title,scope:s,changes:freezeChanges(changes),unresolved:[...unresolved]});}
  function clear(){ui.cancelled=false;const s=scope();plan('Clear selected fixtures',s,matched.map(row=>({row,before:before(row),after:ignored(),command:'set_scene_row'})));}
  async function capture(){
    ui.cancelled=false;const s=scope(),changes:Change[]=[],unresolved:string[]=[];let planned=false;
    try{for(const row of matched){s.fixture(row);ui.progress=`Capturing current output · ${row.item.name}`;ui.touch();const report=await s.inspect(row,slot,true);if(validSetting(report.candidate))changes.push({row,before:before(row),after:report.candidate,command:'set_scene_row'});else unresolved.push(row.item.name);}plan('Review captured output',s,changes,unresolved);planned=true;}
    finally{if(!planned)s.cancelled=true;ui.progress='';ui.touch();}
  }
  async function copy(){
    ui.cancelled=false;const destination=scope(),source=new SceneScope(ui,bus.bus,copySlot);scopes.current.add(source);let planned=false;
    try{if(!source.scene)throw new Error('Choose a named source scene.');const saved=await source.saved();destination.assert();source.assert();plan(`Copy saved scene ${copySlot} → ${slot}`,destination,matched.filter(row=>saved.get(row.key)!==null).map(row=>({row,before:before(row),after:saved.get(row.key)!,command:'set_scene_row'})));planned=true;}finally{source.cancelled=true;if(!planned)destination.cancelled=true;}
  }
  async function apply(){
    const captured=preview;if(!captured)return;setPreview(null);ui.cancelled=false;
    try{await applyChanges(captured.scope,captured.changes,text=>{ui.progress=text;ui.touch();});ui.notice='Reviewed scene settings saved and verified.';}
    finally{captured.scope.cancelled=true;ui.progress='';setReload(n=>n+1);ui.touch();}
  }
  async function verify(all:boolean){
    ui.cancelled=false;const s=scope(),entries:Evidence[]=[];
    try{for(const number of all?Array.from({length:16},(_,i)=>i):[slot])for(const row of s.outputs){s.fixture(row);ui.progress=`Verifying scene ${number} · ${row.item.name}`;ui.touch();const report=await s.inspect(row,number);entries.push({scope:s,row,slot:number,report,candidate:fromStored(report.stored)});}ui.notice='Native scene reads complete. Review differences and unresolved fixtures.';}
    finally{if(entries.length&&!s.cancelled&&s.epoch===ui.client.views.epoch&&s.revision===ui.snapshot.inventory.revision)setReports(entries);else s.cancelled=true;ui.progress='';ui.touch();}
  }
  function review(action:'adopt_scene_row'|'repair_scene_row'){
    if(!reports)return;ui.cancelled=false;
    const relevant=reports.filter(e=>e.slot===slot&&glob(pattern,e.row.item.name));const s=scope();
    for(const entry of relevant){entry.scope.assert();if(entry.scope.revision!==s.revision)throw new Error('Verification is stale. Verify again.');}
    const changes=relevant.filter(e=>e.candidate&&(action==='adopt_scene_row'||e.report.saved!==null)&&e.report.comparison!=='matching').map(e=>({row:e.row,before:before(e.row),after:action==='adopt_scene_row'?e.candidate!:e.report.saved,reviewed:e.report.stored,command:action}));
    closeReports();plan(action==='adopt_scene_row'?'Adopt reviewed native settings':'Repair saved settings',s,changes);
  }
  async function create(){const s=scope();try{s.assert(false);const result=await ui.client.job(itemGeneration(s.initial.inventory.inventory_epoch,s.revision),null,'create_scene',{bus:bus.bus,slot,name:createName||`Scene${slot+1}`,expected_revision:s.revision});await ui.changed(result.revision);if(s.cancelled||s.epoch!==ui.client.views.epoch)return;ui.notice='Scene name saved.';setCreateName('');}finally{s.cancelled=true;}}
  async function recall(){ui.cancelled=false;const s=scope();try{await s.recall(group==='bus'?undefined:Number(group));ui.notice='Scene recall sent. Read fixtures for current output.';}finally{s.cancelled=true;}}
  const actionsDisabled=!ready||!loaded||!scene||!!ui.busy||!matched.length;
  return <><Progress/><div class="scene-mobile-slot"><label>Scene<select aria-label="Scene slot" value={slot} onChange={e=>select(Number(e.currentTarget.value))}>{Array.from({length:16},(_,i)=><option value={i}>{i} · {bus.scenes?.find(s=>s.slot===i)?.name??'Unnamed'}</option>)}</select></label></div><div class="groups-layout scene-layout"><aside class="slots scene-slots"><h2>Scenes</h2>{Array.from({length:16},(_,i)=><button class={i===slot?'selected':''} onClick={()=>select(i)}><span>{i}</span>{bus.scenes?.find(s=>s.slot===i)?.name??'Unnamed'}</button>)}</aside><section>
    <div class="section-heading"><h2>Scene {slot}</h2><button disabled={!ready||!!ui.busy} onClick={()=>void ui.run(()=>verify(true))}>Verify all 16</button></div>
    {scene?<div class="card group-title"><NameField row={scene}/><span class="hint">Slot {slot}</span><button aria-label="Scene actions" onClick={()=>{setDescription(scene.item.description??'');setMore(structuredClone(scene));}}>⋮</button></div>:<div class="card"><h3>Unnamed scene {slot}</h3><p class="muted">Name this slot before saving settings. Browsing leaves the fixtures unchanged.</p><div class="toolbar"><input aria-label="New scene name" placeholder={`Scene${slot+1}`} value={createName} onInput={e=>setCreateName(e.currentTarget.value)}/><button class="primary" disabled={!!ui.busy||!ui.client.views.ready} onClick={()=>void ui.run(create)}>Create scene</button></div></div>}
    <div class="toolbar scene-actions"><button class="primary" disabled={!ready||!scene||!!ui.busy} onClick={()=>void ui.run(recall)}>Recall</button><select aria-label="Recall scope" value={group} disabled={!!ui.busy} onChange={e=>setGroup(e.currentTarget.value)}><option value="bus">Entire bus</option>{Array.from({length:16},(_,i)=><option value={i}>Group {i}{bus.groups?.find(g=>g.group_address===i)?.name?` · ${bus.groups.find(g=>g.group_address===i)!.name}`:''}</option>)}</select><button disabled={!ready||!!ui.busy} onClick={()=>void ui.run(()=>verify(false))}>Verify scene</button></div>
    <div class="toolbar filter scene-filter"><input type="search" aria-label="Scene fixture pattern" placeholder="Filter fixtures · e.g. Light*" value={pattern} onInput={e=>setPattern(e.currentTarget.value)}/><span class="hint">{matched.length} / {outputs.length} fixtures</span></div>
    <div class="toolbar scene-tools"><button disabled={actionsDisabled} onClick={()=>void ui.run(capture)}>Capture current</button><button disabled={actionsDisabled} onClick={()=>void ui.run(async()=>clear())}>Clear selected</button><select aria-label="Copy source scene" value={copySlot} onChange={e=>setCopySlot(Number(e.currentTarget.value))}>{Array.from({length:16},(_,i)=><option value={i}>From {i} · {bus.scenes?.find(s=>s.slot===i)?.name??'Unnamed'}</option>)}</select><button disabled={actionsDisabled||copySlot===slot} onClick={()=>void ui.run(copy)}>Copy saved</button></div>
    <p class="hint scene-explanation">Settings change native scene memory. Recall changes the lights. Blank color attributes keep that value (MASK).</p>
    {loadError&&<p class="error" role="alert">{loadError} <button onClick={()=>setReload(n=>n+1)}>Reload saved settings</button></p>}
    <div class="scene-fixtures">{outputs.map(row=><SceneFixture key={`${slot}:${row.key}`} row={row} visible={glob(pattern,row.item.name)} view={view} makeScope={scope}/>)}</div>{!matched.length&&<p class="empty">No matching fixtures.</p>}
    {preview&&<Modal title={preview.title} close={()=>{preview.scope.cancelled=true;setPreview(null);}}><p>Scene {preview.scope.slot} · {preview.changes.length} fixtures. Current output and unselected fixtures are preserved.</p>{preview.changes.map(c=><section class="verify-row" key={c.row.key}><h3>{c.row.item.name}</h3><p>{summary(c.before)} → {summary(c.after)}</p><details><summary>Native before / after</summary><pre>{JSON.stringify({before:c.before,after:c.after},null,2)}</pre></details></section>)}{!!preview.unresolved.length&&<p class="error">Unresolved; excluded: {preview.unresolved.join(', ')}.</p>}<div class="dialog-actions"><button onClick={()=>{preview.scope.cancelled=true;setPreview(null);}}>Cancel</button><button class="primary" disabled={!!ui.busy||!preview.changes.length} onClick={()=>void ui.run(apply)}>Apply reviewed settings</button></div></Modal>}
    {reports&&<Modal title="Verify native scenes" close={closeReports}><p class="hint">Results cover completed native reads. Adoption and repair below use this slot ({slot}) and the current name filter.</p>{reports.map(e=><section class="verify-row" key={`${e.slot}:${e.row.key}`}><h3>{e.row.item.name} · scene {e.slot}</h3><p>{!e.candidate?'Unresolved':({matching:'Matching',different:'Different',unmanaged:'Unmanaged',unresolved:'Unresolved'} as Record<string,string>)[e.report.comparison]??'Unresolved'}{!e.candidate?' · native data cannot be adopted':''}</p><p class="hint">Saved: {summary(e.report.saved)}</p><details><summary>Native evidence</summary><pre>{JSON.stringify(e.report.stored,null,2)}</pre></details></section>)}<div class="dialog-actions"><button disabled={!scene||!!ui.busy||!loaded} onClick={()=>void ui.run(async()=>review('adopt_scene_row'))}>Review adoption</button><button disabled={!scene||!!ui.busy||!loaded} onClick={()=>void ui.run(async()=>review('repair_scene_row'))}>Review repair</button></div></Modal>}
    {more&&<Modal title={`Scene · ${more.item.name}`} close={()=>setMore(null)}><label class="field">Description<textarea aria-label="Scene description" value={description} onInput={e=>setDescription(e.currentTarget.value)}/></label><div class="dialog-actions"><button disabled={!!ui.busy} onClick={()=>void ui.run(async()=>{await ui.metadata('describe',more.item.target,more.generation,{description:description||null});setMore(null);})}>Save description</button><button disabled={!!ui.busy} onClick={()=>{setMore(null);setRemove(more);}}>Remove scene name</button></div></Modal>}
    {remove&&<Modal title="Remove scene name" close={()=>setRemove(null)}><p>Remove {remove.item.name} and its saved expectations from the bridge. Native fixture contents remain in slot {remove.item.slot}.</p><div class="dialog-actions"><button onClick={()=>setRemove(null)}>Cancel</button><button disabled={!!ui.busy} onClick={()=>void ui.run(async()=>{await ui.remove(remove);setRemove(null);})}>Remove scene</button></div></Modal>}
  </section></div></>;
}
