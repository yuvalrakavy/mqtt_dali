import { render } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import type { TargetedInputEvent } from 'preact';
import './login.css';
function Login(){
  const [identity,setIdentity]=useState({controller:'DALI controller',version:'',build_utc:''});
  const [password,setPassword]=useState(''),[shown,setShown]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
  useEffect(()=>{const abort=new AbortController();fetch('/api/identity',{signal:abort.signal,credentials:'same-origin'}).then(r=>{if(!r.ok)throw new Error();return r.json();}).then(setIdentity).catch(()=>{if(!abort.signal.aborted)setError('Controller identity is unavailable.');});return()=>abort.abort();},[]);
  async function submit(event:Event){event.preventDefault();if(busy||!password)return;setBusy(true);setError('');const abort=new AbortController(),timer=setTimeout(()=>abort.abort(),8000);
    try {const response=await fetch('/api/login',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json'},body:JSON.stringify({password}),signal:abort.signal});
      if(!response.ok){const body=await response.json();throw new Error(body.error??'Sign-in failed.');}
      setPassword('');location.assign('/');
    }catch(error){setError((error as Error).message==='The operation was aborted.'?'The controller did not answer.':(error as Error).message);setBusy(false);}finally{clearTimeout(timer);}
  }
  const input={id:'password',name:'password',autoComplete:'current-password',value:password,onInput:(e:TargetedInputEvent<HTMLInputElement>)=>setPassword(e.currentTarget.value),required:true,autoFocus:true,disabled:busy};
  return <main class="signin"><div class="brand">DALI</div><section class="signin-card"><p class="eyebrow">CONTROLLER CONFIGURATION</p><h1>{identity.controller}</h1><p class="muted">Sign in to configure buses and devices.</p><form onSubmit={submit}><label for="password">Password</label><div class="password">{shown?<input type="text" {...input}/>:<input type="password" {...input}/>}<button type="button" onClick={()=>setShown(!shown)} aria-label={shown?'Hide password':'Show password'} title={shown?'Hide password':'Show password'} aria-controls="password"><svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>{shown&&<path d="m3 3 18 18"/>}</svg></button></div><button class="primary" type="submit" disabled={busy||!password}>{busy?'Signing in…':'Sign in'}</button>{error&&<p class="error" role="alert">{error}</p>}</form><p class="foot">Configuration access is protected. Normal lighting control continues independently.</p></section><footer>mqtt_dali {identity.version}<br/>Build {identity.build_utc||'—'}</footer></main>;
}
render(<Login/>,document.getElementById('root')!);
