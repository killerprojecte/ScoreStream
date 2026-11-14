import type { Config } from 'tailwindcss';
import animate from 'tailwindcss-animate';
export default {
 darkMode:['class','[data-theme="dark"]'],content:['./index.html','./src/**/*.{ts,tsx}'],
 theme:{extend:{colors:{'glass-ink':'rgb(var(--g-ink) / <alpha-value>)','glass-muted':'rgb(var(--g-secondary-ink) / <alpha-value>)','glass-accent':'rgb(var(--g-accent) / <alpha-value>)',...Object.fromEntries(['border','input','ring','background','foreground','primary','secondary','destructive','muted','accent','popover','card'].map(key=>[key,['primary','secondary','destructive','muted','accent','popover','card'].includes(key)?{DEFAULT:`hsl(var(--${key}))`,foreground:`hsl(var(--${key}-foreground))`}:`hsl(var(--${key}))`]))},borderRadius:{lg:'var(--radius)',md:'calc(var(--radius) - 2px)',sm:'calc(var(--radius) - 4px)'}}},plugins:[animate],
} satisfies Config;
