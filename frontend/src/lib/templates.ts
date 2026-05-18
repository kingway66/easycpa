const MODEL_TEMPLATES = [
  { name: 'gpt-5.5', model: 'gpt-5.5', api_format: 'openai_chat', icon: '🔵', label: 'GPT-5.5' },
  { name: 'gpt-5.4', model: 'gpt-5.4', api_format: 'openai_responses', icon: '🔵', label: 'GPT-5.4' },
  { name: 'claude-opus-4-7', model: 'claude-opus-4-7', api_format: 'anthropic', icon: '🟠', label: 'Opus 4.7' },
  { name: 'claude-opus-4-6', model: 'claude-opus-4-6', api_format: 'anthropic', icon: '🟠', label: 'Opus 4.6' },
  { name: 'claude-sonnet-4-6', model: 'claude-sonnet-4-6', api_format: 'anthropic', icon: '🟠', label: 'Sonnet 4.6' },
  { name: 'claude-haiku-4-5-20251001', model: 'claude-haiku-4-5-20251001', api_format: 'anthropic', icon: '🟠', label: 'Haiku 4.5' },
  { name: 'deepseek-v4-pro', model: 'deepseek-v4-pro', api_format: 'openai_chat', icon: '🟣', label: 'DeepSeek V4 Pro' },
  { name: 'deepseek-v4-flash', model: 'deepseek-v4-flash', api_format: 'openai_chat', icon: '🟣', label: 'DeepSeek V4 Flash' },
]

export function getTemplateIcon(name: string): string {
  return MODEL_TEMPLATES.find(t => t.name === name)?.icon || '⚪'
}

export function getTemplateLabel(name: string): string {
  return MODEL_TEMPLATES.find(t => t.name === name)?.label || name
}

export default MODEL_TEMPLATES
