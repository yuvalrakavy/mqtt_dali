const channels=['ColorRed','ColorGreen','ColorBlue','ColorWhite','ColorAmber','ColorFree'];
export const colorLabel=(field:string):string=>({ColorTemperature:'Temperature (K)',ColorX:'x',ColorY:'y',ColorRed:'Red (%)',ColorGreen:'Green (%)',ColorBlue:'Blue (%)',ColorWhite:'White (%)',ColorAmber:'Amber (%)',ColorFree:'Free color (%)'}[field]??field);
export function colorFields(representation:string,assignments:unknown):string[]{
  if(representation==='temperature')return ['ColorTemperature'];
  if(representation==='xy')return ['ColorX','ColorY'];
  return representation==='rgbwaf'&&Array.isArray(assignments)?channels.filter(field=>assignments.includes(field.slice(5).toLowerCase())):[];
}
export function colorActual(state:Record<string,unknown>,field:string,representation:string):string{
  const value=state[field];
  return typeof value==='number'?String(Math.round(value*(representation==='rgbwaf'?100:1)*10000)/10000):state[`${field}Mixed`]===true?'Mixed':'Unknown';
}
export function colorPatch(draft:Record<string,string>,fields:string[],representation:string):Record<string,number>{
  const patch:Record<string,number>={};
  for(const field of fields){
    const text=draft[field];if(text===undefined||text.trim()==='')continue;
    const value=Number(text),max=representation==='xy'?65534/65536:representation==='rgbwaf'?100:1_000_000;
    const min=representation==='temperature'?1_000_000/65534:0;
    if(!Number.isFinite(value)||value<min||value>max)throw new Error(`Enter a valid ${colorLabel(field)} value.`);
    patch[field]=representation==='rgbwaf'?value/100:value;
  }
  if(!Object.keys(patch).length)throw new Error('Change at least one color value.');
  return patch;
}
