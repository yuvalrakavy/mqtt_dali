import {useState} from 'preact/hooks';
import {busReady,values,type PhysicalRow} from './model.ts';
import {fixtureStatus} from './fixture-status.ts';
import {Modal,useUi} from './ui.tsx';
export function FixtureStatus({row}:{row:PhysicalRow}){
  const ui=useUi(),[open,setOpen]=useState(false),current=ui.physical(row.key);
  const status=fixtureStatus(values(ui.snapshot,current));
  const ready=ui.client.views.ready&&busReady(ui.snapshot,current.bus);
  return <>{status.severity!=='clear'&&<button type="button" class={`fixture-status ${status.severity}`} aria-label={`Fixture status ${current.item.name}`} title={status.summary} onClick={()=>setOpen(true)}>
    {status.severity==='unknown'?<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 4 2c-1 .7-1.5 1-1.5 2.5M12 16v1"/></svg>:<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 3 2 21h20L12 3Z M12 9v5 M12 17v1"/></svg>}
  </button>}{open&&<Modal title={`Fixture status · ${current.item.name}`} close={()=>setOpen(false)}><p>{status.summary}</p><p class="hint">Status: {status.quality} · Raw: {status.raw===null?'unknown':`0x${status.raw.toString(16).toUpperCase().padStart(2,'0')}`}</p><dl class="fixture-status-list">{status.flags.map(flag=><><dt key={`${flag.key}-label`}>{flag.label}</dt><dd key={flag.key}>{flag.value===null?'Unknown':flag.value?'Yes':'No'}</dd></>)}</dl><p class="hint">Reported by the fixture on its last read. This does not measure physical illumination.</p><div class="dialog-actions"><button disabled={!ready||!!ui.busy} onClick={()=>void ui.run(()=>ui.read(current))}>Read current values</button></div></Modal>}</>;
}
