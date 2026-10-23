// Producer: dali::luminaire::Data in the read_luminaire_data report.
export interface LuminaireField {status:'value'|'unknown'|'unavailable'|'invalid';value:number|string|null;unit:string|null}
export interface LuminaireData {format_id:number|null;format_status:'supported'|'unrecognized'|'unavailable';fields:Record<string,LuminaireField>}
export const luminaireFields=[
  ['manufacturer_gtin','Luminaire GTIN'],['luminaire_id','Luminaire ID'],['manufacture_year','Manufacture year (YY)'],['manufacture_week','Manufacture week'],
  ['input_power_nominal','Nominal input power'],['input_power_minimum_dim','Power at minimum level'],['mains_voltage_minimum','Minimum AC voltage rating'],['mains_voltage_maximum','Maximum AC voltage rating'],
  ['light_output_nominal','Nominal light output'],['cri','CRI'],['cct','Nominal CCT'],['light_distribution','Light distribution'],['oem_color','OEM color text'],['identification','Identification text']
] as const;
const units:Record<string,string|null>={manufacturer_gtin:null,luminaire_id:null,manufacture_year:null,manufacture_week:null,input_power_nominal:'W',input_power_minimum_dim:'W',mains_voltage_minimum:'V',mains_voltage_maximum:'V',light_output_nominal:'lm',cri:null,cct:'K',light_distribution:null,oem_color:null,identification:null};
const bounds:Record<string,[number,number]>={manufacture_year:[0,99],manufacture_week:[1,53],input_power_nominal:[0,65534],input_power_minimum_dim:[0,65534],mains_voltage_minimum:[90,480],mains_voltage_maximum:[90,480],light_output_nominal:[0,0xfffffe],cri:[0,100],cct:[0,17000]};
const invalid=():never=>{throw new Error('Luminaire data is incomplete or invalid. Read again.');};
function field(key:string,f:any){
  if(!f||!['value','unknown','unavailable','invalid'].includes(f.status))return invalid();
  const unit=key==='cct'&&f.status==='value'&&f.value==='part_209'?null:units[key];if(f.unit!==unit)return invalid();
  if(f.status!=='value'){if(f.value!==null)return invalid();return;}
  const v=f.value;
  if(key==='cct'&&v==='part_209')return;
  if(key in bounds){const [min,max]=bounds[key];if(typeof v!=='number'||!Number.isInteger(v)||v<min||v>max)return invalid();return;}
  if(typeof v!=='string')return invalid();
  if(key==='manufacturer_gtin'){if(!/^\d{14}$/.test(v))return invalid();}
  else if(key==='luminaire_id'){if(!/^[0-9A-F]{16}$/.test(v))return invalid();}
  else if(key==='light_distribution'){if(!['not_specified','type_i','type_ii','type_iii','type_iv','type_v'].includes(v))return invalid();}
  else if(v.length>(key==='oem_color'?24:60)||[...v].some(c=>c.charCodeAt(0)===0||c.charCodeAt(0)>127))return invalid();
}
export function decodeLuminaire(raw:any,bytes:(number|null)[]):{data:LuminaireData;partial:boolean}{
  if(!raw||!raw.fields||typeof raw.fields!=='object'||Array.isArray(raw.fields)||bytes.length<1||bytes.length>120)return invalid();
  const header=bytes[0];
  if(header===null||header===255){
    if(bytes.length!==1||raw.format_id!==null||raw.format_status!=='unavailable'||Object.keys(raw.fields).length!==0)return invalid();
    return {data:structuredClone(raw),partial:header===255};
  }
  const high=bytes[17]??null,low=bytes[18]??null,format=high===null||low===null?null:(high<<8)|low,status=format===3?'supported':format===null?'unavailable':'unrecognized';
  if(raw.format_id!==format||raw.format_status!==status||bytes.length!==Math.min(header,format===3?119:18)+1)return invalid();
  if(format===3){
    if(Object.keys(raw.fields).length!==luminaireFields.length)return invalid();
    for(const [key]of luminaireFields)field(key,raw.fields[key]);
  }else if(Object.keys(raw.fields).length!==0)return invalid();
  return {data:structuredClone(raw),partial:format!==3||header<119};
}
export function luminaireDisplay(key:string,f:LuminaireField):string{
  if(f.status!=='value')return f.status==='invalid'?'Invalid data':f.status==='unavailable'?'Unavailable':'Unknown';
  if(key==='cct'&&f.value==='part_209')return 'Variable (Part 209 indication)';
  if(key==='light_distribution')return ({not_specified:'Not specified',type_i:'Type I',type_ii:'Type II',type_iii:'Type III',type_iv:'Type IV',type_v:'Type V'} as Record<string,string>)[String(f.value)];
  const value=key==='manufacture_year'?String(f.value).padStart(2,'0'):f.value===''?'Not specified':String(f.value);
  return value+(f.unit?` ${f.unit}`:'');
}
