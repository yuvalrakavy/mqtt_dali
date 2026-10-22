import { createContext, type ComponentChildren } from 'preact';
import { useContext, useEffect, useRef } from 'preact/hooks';
import { Controller } from './controller.ts';
import { Draft, type Row } from './model.ts';
export const Context=createContext<Controller|null>(null);
export function useUi(){const ui=useContext(Context);if(!ui)throw new Error('Controller is unavailable.');return ui;}
export function Progress(){const ui=useUi();return ui.progress?<div class="progress" role="status"><span>{ui.progress}</span><button onClick={()=>{ui.cancelled=true;ui.touch();}} disabled={ui.cancelled}>{ui.cancelled?'Stopping…':'Stop after current step'}</button></div>:null;}
export function navigate(path:string){history.pushState(null,'',path);dispatchEvent(new PopStateEvent('popstate'));}
export function Link(props:{href:string;children:ComponentChildren;class?:string}){return <a href={props.href} class={props.class} onClick={e=>{if(e.metaKey||e.ctrlKey||e.shiftKey||e.altKey)return;e.preventDefault();navigate(props.href);}}>{props.children}</a>;}
export function Modal(props:{title:string;close:()=>void;children:ComponentChildren}){
  const ui=useUi();
  const dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{dialog.current?.showModal();return()=>dialog.current?.close();},[]);
  return <dialog ref={dialog} onCancel={e=>{e.preventDefault();props.close();}} onClick={e=>{if(e.target===e.currentTarget)props.close();}}><div class="dialog-body"><header><h2>{props.title}</h2><button type="button" onClick={props.close} aria-label="Close dialog">×</button></header>{props.children}{ui.error&&<p class="error" role="alert">{ui.error}</p>}</div></dialog>;
}
export function NameField({row}:{row:Row}){
  const ui=useUi();const draft=ui.drafts.get(row.key);
  const commit=()=>{const draft=ui.drafts.get(row.key);if(!draft)return;void draft.commit((text,baseline)=>ui.rename(row.key,text,baseline)).catch(()=>{}).finally(()=>ui.touch());ui.touch();};
  const input=useRef<HTMLInputElement>(null);
  return <div class="name-cell"><div class={`name-edit ${draft?.dirty?'dirty':''}`}><input ref={input} class="name-field" aria-label={`Name ${row.item.name}`} value={draft?.text??row.item.name} disabled={!!draft?.pending||!!ui.progress} autoComplete="off" autoCapitalize="off" spellcheck={false}
    onInput={e=>{let d=ui.drafts.get(row.key);if(!d){d=new Draft(row.item.name,row.item.target,row.generation);ui.drafts.set(row.key,d);}d.text=e.currentTarget.value;d.error='';ui.touch();}}
    onBlur={commit} onKeyDown={e=>{if(e.key==='Enter'){e.preventDefault();commit();}if(e.key==='Escape'){ui.drafts.delete(row.key);ui.touch();input.current?.blur();}}}/>
    {draft?.dirty&&<button type="button" aria-label={`Save name ${row.item.name}`} disabled={!!draft.pending} onPointerDown={e=>e.preventDefault()} onClick={commit}>✓</button>}</div>
    {draft?.pending&&<p class="hint" role="status">Saving…</p>}
    {draft?.error&&<p class="error" role="alert">{draft.error} <button class="text-button" onPointerDown={e=>e.preventDefault()} onClick={()=>{ui.drafts.delete(row.key);ui.touch();}}>Discard draft</button></p>}
  </div>;
}
