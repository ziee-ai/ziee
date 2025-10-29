import { FaServer, FaWrench } from 'react-icons/fa'
import {
  RiOpenaiFill,
  RiAnthropicFill,
  RiGeminiFill,
} from 'react-icons/ri'
import { BsFillLightningChargeFill } from 'react-icons/bs'
import { SiHuggingface } from 'react-icons/si'
import { DeepSeek, Mistral } from '@lobehub/icons'

export const PROVIDER_ICONS: Record<string, any> = {
  local: FaServer,
  openai: RiOpenaiFill,
  anthropic: RiAnthropicFill,
  groq: BsFillLightningChargeFill,
  gemini: RiGeminiFill,
  mistral: Mistral,
  deepseek: DeepSeek,
  huggingface: SiHuggingface,
  custom: FaWrench,
}
