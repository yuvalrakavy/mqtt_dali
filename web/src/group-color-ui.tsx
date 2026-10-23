import {useState} from 'preact/hooks';
import {busReady,values,type PhysicalRow} from './model.ts';
import {Modal,useUi} from './ui.tsx';
import {colorActual,colorFields,colorLabel,colorPatch} from './group-color.ts';

export function GroupColor({row}:{row:PhysicalRow}){
  const ui=useUi(),current=ui.physical(row.key),state=values(ui.snapshot,current);
  const [open,setOpen]=useState(false),[draft,setDraft]=useState<Record<string,string>>({});
  const representation=current.item.color_representation??'';
  const caps=state.ColorCapabilities as {supported_representations?:string[]|null;common_rgbwaf_assignments?:string[]|null;temperature_limits?:{coolest_kelvin:number;warmest_kelvin:number}|null}|null;
  const supported=caps?.supported_representations?.filter(value=>['temperature','xy','rgbwaf'].includes(value));
  const fields=colorFields(representation,caps?.common_rgbwaf_assignments);
  const identification=ui.snapshot.identification;
  const blinking=identification&&['starting','blinking','restoring'].includes(identification.phase)&&ui.currentRows.some(item=>item.item.target===identification.target&&item.bus.bus===row.bus.bus);
  const ready=busReady(ui.snapshot,current.bus)&&ui.client.views.ready&&!ui.busy;
  const editable=ready&&!blinking&&supported?.includes(representation)&&!!fields.length;
  const read=()=>void ui.run(()=>ui.read(current));
  return <section class="card" aria-label="Group color">
    <div class="toolbar"><strong>Color</strong><span class="muted">{state.ColorQuality??'Unobserved'}</span><span class="spacer"/><button disabled={!ready} onClick={read}>Inspect color support</button></div>
    <p class="hint">Common support: {supported?supported.length?supported.join(', '):'no common color representation':'unknown — inspect hardware members'}{caps?.temperature_limits?` · ${Math.round(caps.temperature_limits.warmest_kelvin)}–${Math.round(caps.temperature_limits.coolest_kelvin)} K`:''}. Covers saved fixtures.</p>
    <div class="toolbar"><label class="field">Representation<select aria-label="Group color representation" value={representation} disabled={!ready||!!blinking||!supported} onChange={event=>{const value=event.currentTarget.value;void ui.run(()=>ui.representation(current,value));}}><option value="">None selected</option>{[...new Set([...(supported??[]),...(representation?[representation]:[])])].map(value=><option key={value} value={value} disabled={!supported?.includes(value)}>{value==='temperature'?'White temperature':value==='xy'?'CIE xy':'RGBWAF channels'}</option>)}</select></label><button disabled={!editable} onClick={()=>{setDraft({});setOpen(true);}}>Group color values</button></div>
    {supported?.length===0&&<p class="hint">One or more hardware members do not support a common color format. Review group membership.</p>}
    {!!blinking&&<p class="hint">Stop blinking before changing group color.</p>}
    {open&&<Modal title={`Color · ${current.item.name}`} close={()=>setOpen(false)}><p class="hint">Color status: {String(state.ColorQuality??'unknown')}. Mixed values remain mixed until you change them.</p><div class="colour-fields">{fields.map(field=>{const actual=colorActual(state,field,representation);return <label class="field" key={field}>{colorLabel(field)}<input type="number" aria-label={colorLabel(field)} min={representation==='temperature'?1:0} max={representation==='rgbwaf'?100:representation==='xy'?65534/65536:undefined} step="any" disabled={!editable} placeholder={actual} value={draft[field]??(typeof state[field]==='number'?actual:'')} onInput={event=>setDraft({...draft,[field]:event.currentTarget.value})}/><span class="hint">Actual: {actual}</span></label>;})}</div><div class="dialog-actions"><button disabled={!ready} onClick={read}>Read current values</button><button class="primary" disabled={!editable} onClick={()=>void ui.run(async()=>{await ui.groupColor(current,colorPatch(draft,fields,representation));setOpen(false);})}>Apply group color</button></div></Modal>}
  </section>;
}
