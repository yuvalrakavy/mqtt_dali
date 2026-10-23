import { FixtureStatus } from './fixture-status-ui.tsx';
import { InputSettings } from './input-settings-ui.tsx';
import { OutputSettings } from './output-settings-ui.tsx';
import { Feedback } from './feedback-ui.tsx';
import { useEffect, useLayoutEffect, useState } from 'preact/hooks';
import { busReady, glob, inputType, inputDisplay, values, type Bus, type PhysicalRow as Row } from './model.ts';
import { Modal, NameField, Progress, useUi } from './ui.tsx';
import { ClearAddress, Restoration, type RestorationReview } from './restoration.ts';
import { Recovery } from './restoration-ui.tsx';

function Colour({row,close}:{row:Row;close:()=>void}){
  const ui=useUi(),current=ui.physical(row.key),state=values(ui.snapshot,current);
  const representation=current.item.color_representation??'';
  const capabilities=state.ColorCapabilities as {supported_representations?:string[]|null;rgbwaf_channel_count?:number|null;temperature_limits?:{coolest_kelvin?:number|null;warmest_kelvin?:number|null}|null}|null;
  const supported=capabilities?.supported_representations,limits=capabilities?.temperature_limits;
  const fields=representation==='temperature'?['ColorTemperature']:representation==='xy'?['ColorX','ColorY']:representation==='rgbwaf'?['ColorRed','ColorGreen','ColorBlue','ColorWhite','ColorAmber','ColorFree']:[];
  const [draft,setDraft]=useState<Record<string,string>>({});
  const label=(field:string)=>({ColorTemperature:'Temperature (K)',ColorX:'x',ColorY:'y',ColorRed:'Red (%)',ColorGreen:'Green (%)',ColorBlue:'Blue (%)',ColorWhite:'White (%)',ColorAmber:'Amber (%)',ColorFree:'Free color (%)'}[field as 'ColorX']);
  const scaled=(field:string,value:unknown)=>typeof value==='number'?(representation==='rgbwaf'?value*100:value).toString():'';
  const apply=()=>ui.run(async()=>{const properties:Record<string,number>={};for(const field of fields){const value=draft[field];if(value!==undefined&&value!==''){const number=Number(value);if(!Number.isFinite(number))throw new Error('Enter numeric color values.');properties[field]=representation==='rgbwaf'?number/100:number;}}
    if(!Object.keys(properties).length)throw new Error('Change at least one color value.');await ui.apply(current,properties);close();});
  return <Modal title={`Color · ${current.item.name}`} close={close}><label class="field">Representation<select aria-label="Color representation" value={representation} disabled={!!ui.busy} onChange={e=>{const value=e.currentTarget.value;void ui.run(async()=>{await ui.representation(current,value);close();});}}><option value="">None selected</option><option value="temperature">White temperature</option><option value="xy">CIE xy</option><option value="rgbwaf">RGBWAF channels</option></select></label>
    <p class="hint">Choose the values this fixture uses. Selecting a representation does not change its physical operating mode.</p>
    <p class="hint">Observed support: {supported?supported.length?supported.join(', '):'no supported color representations':'unknown'}{capabilities?.rgbwaf_channel_count?` · ${capabilities.rgbwaf_channel_count} RGBWAF channels`:''}{limits?.warmest_kelvin&&limits.coolest_kelvin?` · ${Math.round(limits.warmest_kelvin)}–${Math.round(limits.coolest_kelvin)} K`:''}</p>
    <div class="colour-fields">{fields.map(field=><label class="field" key={field}>{label(field)}<input type="number" step={representation==='temperature'?1:'any'} min={representation==='temperature'?1:0} max={representation==='rgbwaf'?100:representation==='xy'?1:undefined} aria-label={label(field)} value={draft[field]??scaled(field,state[field])} placeholder="Unknown" onInput={e=>setDraft({...draft,[field]:e.currentTarget.value})}/><span class="hint">Actual: {scaled(field,state[field])||'unknown'}</span></label>)}</div>
    <p class="hint">{state.ColorQuality===undefined?'Read the fixture to discover color support.':`Color status: ${state.ColorQuality}`}</p><div class="dialog-actions"><button disabled={!!ui.busy} onClick={()=>void ui.run(()=>ui.read(current))}>Read values</button><button class="primary" disabled={!!ui.busy||!fields.length} onClick={apply}>Apply color</button></div></Modal>;
}
function FixtureGroups({row,close}:{row:Row;close:()=>void}){
  const ui=useUi(),current=ui.physical(row.key);
  return <Modal title={`Groups · ${current.item.name}`} close={close}><p class="hint">Saved membership. Verify groups to compare with hardware.</p><div class="checklist">{(current.bus.groups??[]).map(group=><label key={group.group_address}><input type="checkbox" checked={group.members?.includes(current.item.short_address)??false} disabled={!!ui.busy} onChange={e=>{const checked=e.currentTarget.checked;void ui.run(()=>ui.group(current,group.group_address!,checked));}}/>{group.name} <span class="hint">{group.group_address}</span></label>)}{!current.bus.groups?.length&&<p>No groups are saved. Open the Groups tab to create one.</p>}</div></Modal>;
}
function DeviceRow({row,recover,select}:{row:Row;recover:(row:Row)=>void;select?:()=>void}){
  const ui=useUi(),snapshot=ui.snapshot,state=values(snapshot,row);
  const [clear,setClear]=useState<ClearAddress|null>(null);
  useLayoutEffect(()=>()=>{if(clear)clear.scope.cancelled=true;},[clear]);
  const closeClear=()=>{if(clear)clear.scope.cancelled=true;setClear(null);};
  const [inputSettings,setInputSettings]=useState(false);
  const [outputSettings,setOutputSettings]=useState(false);
  const [feedback,setFeedback]=useState(false);
  const [menu,setMenu]=useState(false),[colour,setColour]=useState(false),[groups,setGroups]=useState(false),[remove,setRemove]=useState(false),[level,setLevel]=useState<string|null>(null);
  const ready=busReady(snapshot,row.bus)&&ui.client.views.ready;
  const identified=snapshot.identification?.target===row.item.target?snapshot.identification:null;
  const blinking=!!identified&&['starting','blinking','restoring'].includes(identified.phase);
  const activity=ui.client.events.find(e=>e.target===row.item.target&&e.generation===row.generation);
  const hints=ui.hints.get(row.key),type=row.kind==='output'?(hints?.length?hints.map(t=>`DT${t}`).join(', '):'Output'):row.kind==='input'?'Input module':inputType(row.item.instance_type);
  const dt8=(state.ColorCapabilities as {dt8?:boolean|null}|null|undefined)?.dt8;
  const hasColor=row.kind==='output'&&(dt8===true||(dt8!==false&&hints?.includes(8)===true));
  const inputState=inputDisplay(state);
  const percent=typeof state.Brightness==='number'?Math.round(state.Brightness*100):null;
  return <div class={`device-row ${row.kind==='output'?'output-grid':'input-grid'} ${blinking?'identifying':''} ${activity&&Date.now()-activity.received<5000?'active-input':''}`} data-device-key={row.key}>
    <span class="address">{row.parent?`${row.parent.short_address} / ${row.item.number}`:row.item.short_address}</span>{row.kind==='output'?<div class="fixture-name"><NameField row={row}/><FixtureStatus row={row}/></div>:<NameField row={row}/>}<span class="device-type">{type}</span>
    {row.kind==='output'?<><button class={`power ${state.PowerState===true?'on':state.PowerState===false?'off':'unknown'}`} aria-label={`Power ${row.item.name}`} aria-pressed={typeof state.PowerState==='boolean'?state.PowerState:'mixed'} disabled={!ready||!!ui.busy||blinking} onClick={()=>void ui.run(()=>ui.apply(row,{PowerState:state.PowerState!==true}))}>{state.PowerState===true?'On':state.PowerState===false?'Off':'?'}</button>
      <div class="brightness"><input type="range" min="0" max="100" value={level??percent??0} aria-label={`Brightness ${row.item.name}`} disabled={!ready||!!ui.busy||blinking||percent===null} onInput={e=>setLevel(e.currentTarget.value)} onChange={e=>{const brightness=Number(e.currentTarget.value)/100;void ui.run(async()=>{try{await ui.apply(row,{Brightness:brightness});}finally{setLevel(null);}});}}/><output>{level??percent??'—'}{percent!==null?'%':''}</output></div>
      {hasColor?<button class="colour-button" aria-label={`Color ${row.item.name}`} disabled={!ready||!!ui.busy||blinking} onClick={()=>setColour(true)}>{row.item.color_representation==='temperature'&&typeof state.ColorTemperature==='number'?`${Math.round(state.ColorTemperature)} K`:row.item.color_representation==='xy'?'xy controls':row.item.color_representation==='rgbwaf'?'RGB controls':'Color'}</button>:<span class="colour-button" aria-hidden="true"/>}
      <button class={blinking?'primary blink':'blink'} aria-label={`${blinking?'Stop blink':'Blink'} ${row.item.name}`} disabled={!ready||!!ui.busy||identified?.phase==='restoring'} onClick={()=>void ui.run(()=>ui.blink(row))}>{identified?.phase==='restoring'?'Restoring…':blinking?'Stop blink':'Blink'}</button></>:<><span class={`input-state ${state.Available===true?'':'muted'}`}>{row.kind==='input'?`${row.item.instances?.length??0} instances`:inputState}</span><span class="activity">{activity?.action??'No recent action'}</span></>}
    <button class="menu-button" aria-label={`Actions ${row.item.name}`} onClick={()=>setMenu(true)}>⋮</button>
    {menu&&<Modal title={row.item.name} close={()=>setMenu(false)}><div class="menu-actions">
      {row.kind==='output'&&<button disabled={!ready||!!ui.busy} onClick={()=>{ui.cancelled=false;setMenu(false);setOutputSettings(true);}}>Fixture settings</button>}
      {row.kind==='instance'&&<button disabled={!ready||!!ui.busy} onClick={()=>{ui.cancelled=false;setMenu(false);setInputSettings(true);}}>Event settings</button>}
      {row.kind!=='output'&&<button disabled={!ready||!!ui.busy} onClick={()=>{ui.cancelled=false;setMenu(false);setFeedback(true);}}>Feedback</button>}
      {row.kind!=='input'&&<button disabled={!ready||!!ui.busy} onClick={()=>{setMenu(false);void ui.run(()=>ui.read(row));}}>Read current values</button>}
      {row.kind==='output'&&<button onClick={()=>{setMenu(false);setGroups(true);}}>Group membership</button>}
      {hasColor&&<button onClick={()=>{setMenu(false);setColour(true);}}>Color representation and values</button>}
      {row.kind!=='instance'&&<button disabled={!ready||!!ui.busy} onClick={()=>{setMenu(false);recover(row);}}>Restore lost address</button>}
      {row.kind!=='instance'&&<button class="danger" disabled={!ready||!!ui.busy} onClick={()=>{ui.cancelled=false;setMenu(false);setClear(new ClearAddress(ui,row));}}>Clear short address</button>}
      {row.kind!=='instance'&&<button class="danger" onClick={()=>{setMenu(false);setRemove(true);}}>Remove from list</button>}
    </div></Modal>}
    {inputSettings&&<InputSettings row={row} close={()=>setInputSettings(false)}/>}
    {outputSettings&&<OutputSettings row={row} close={()=>setOutputSettings(false)}/>}
    {feedback&&<Feedback row={row} close={()=>setFeedback(false)}/>}
    {clear&&<Modal title="Clear short address?" close={closeClear}><p>Clear address <strong>{clear.scope.original.item.short_address}</strong> from <strong>{clear.scope.original.item.name}</strong> on {clear.scope.initial.inventory.name}/{clear.scope.original.bus.name}.</p><p>The saved name, bindings and address reservation stay. The fixture will be unaddressed. Use Add unaddressed, then Restore lost address to identify it again.</p><div class="dialog-actions"><button onClick={closeClear}>Cancel</button><button class="danger" disabled={!!ui.busy} onClick={()=>void ui.run(async()=>{await clear.run();setClear(null);})}>{blinking?'Stop blink and clear address':'Clear address'}</button></div></Modal>}
    {select&&<div class="row-error"><button disabled={!ready||!!ui.busy} onClick={select}>Use this fixture</button></div>}
    {colour&&hasColor&&<Colour row={row} close={()=>setColour(false)}/>}{groups&&<FixtureGroups row={row} close={()=>setGroups(false)}/>}
    {remove&&<Modal title={`Remove ${row.item.name}?`} close={()=>setRemove(false)}><p>Remove this {row.kind==='input'?'module and all its instances':'fixture'} from the saved list. Its hardware address stays unchanged and discovery can find it again.</p><div class="dialog-actions"><button onClick={()=>setRemove(false)}>Cancel</button><button class="danger" disabled={!!ui.busy} onClick={()=>void ui.run(async()=>{await ui.remove(row);setRemove(false);})}>Remove from list</button></div></Modal>}
    {identified?.phase==='interrupted'&&<p class="hint row-error">Identification interrupted. Restoration was not confirmed.</p>}
  </div>;
}
export function Devices({bus}:{bus:Bus}){
  const ui=useUi(),[filter,setFilter]=useState('');
  const [recovery,setRecovery]=useState<Restoration|null>(null),[review,setReview]=useState<RestorationReview|null>(null);
  useEffect(()=>()=>{if(recovery)recovery.cancelled=true;},[recovery]);
  const recover=(row:Row)=>{ui.cancelled=false;setReview(null);setFilter('');setRecovery(new Restoration(ui,row));};
  const select=(row:Row)=>recovery?.candidates.has(row.key)?()=>{try{setReview(recovery.review(row));}catch(error){ui.error=(error as Error).message;ui.touch();}}:undefined;
  const all=ui.currentRows.filter((r):r is Row=>r.kind!=='scene'&&r.bus.bus===bus.bus);
  const visible=(row:Row)=>glob(filter,row.item.name)||!!ui.drafts.get(row.key)?.dirty||!!ui.drafts.get(row.key)?.error;
  const outputs=all.filter(r=>r.kind==='output'&&visible(r)),inputs=all.filter(r=>(r.kind==='input'||r.kind==='instance')&&visible(r));
  const ready=busReady(ui.snapshot,bus)&&ui.client.views.ready;
  const retired=[...ui.drafts].filter(([key,d])=>d.dirty&&!ui.currentRows.some(r=>r.key===key));
  return <><div class="toolbar discovery"><button class="primary" disabled={!ready||!!ui.busy} onClick={()=>void ui.run(()=>ui.scan(bus,true))}>Add unaddressed</button><button disabled={!ready||!!ui.busy} onClick={()=>void ui.run(()=>ui.scan(bus,false))}>Refresh addressed</button><span class="hint">Existing names and addresses are preserved.</span></div>
    <Progress/>
    {recovery&&<Recovery scope={recovery} review={review} choose={setReview} close={()=>{setReview(null);setRecovery(null);}}/>}
    <div class="toolbar filter"><input type="search" value={filter} placeholder="Filter names · e.g. Light*" aria-label="Filter device names" onInput={e=>setFilter(e.currentTarget.value)}/>{['','Light*','Input*'].map(value=><button class={filter===value?'soft':''} onClick={()=>setFilter(value)}>{value||'All'}</button>)}<span class="spacer"/><span class="hint">{all.filter(r=>r.kind==='output').length} outputs · {all.filter(r=>r.kind==='input').length} input modules</span></div>
    <div class="section-heading"><h2>Outputs</h2><span class="hint">✓, Enter or leave the field to save</span></div><div class="device-table"><div class="table-heading output-grid"><span>Address</span><span>Name</span><span>Type</span><span>Power</span><span>Brightness</span><span>Color</span><span>Identify</span><span/></div>{outputs.map(row=><DeviceRow key={row.key} row={row} recover={recover} select={select(row)}/>)}{!outputs.length&&<p class="empty">{filter?'No matching outputs.':'No outputs are saved. Start discovery to find the fixtures on this bus.'}</p>}</div>
    <div class="section-heading"><h2>Inputs</h2><span class="hint">Operate a switch to see its current state and action.</span></div><div class="device-table"><div class="table-heading input-grid"><span>Addr / instance</span><span>Name</span><span>Type</span><span>Current state</span><span>Last action</span><span/></div>{inputs.map(row=><DeviceRow key={row.key} row={row} recover={recover} select={select(row)}/>)}{!inputs.length&&<p class="empty">{filter?'No matching inputs.':'No input modules are saved.'}</p>}</div>
    {retired.map(([key,draft])=><div class="card retired" key={key}><p>Removed or replaced fixture · unsaved name: <strong>{draft.text}</strong></p><p class="error">This draft cannot be applied to a replacement.</p><button onClick={()=>{ui.drafts.delete(key);ui.touch();}}>Discard retired draft</button></div>)}
    {!ready&&<p class="hint">{!bus.enabled?'Enable this bus to use hardware controls.':'Hardware controls are unavailable until the bus is ready.'}</p>}
  </>;
}
