import { Title } from '@/components/ui'

import {
  LlmModelParameterField,
  ParameterFieldConfig,
} from '@/modules/llm-provider/components/llm-models/shared/LlmModelParameterField'

interface LlmModelParametersSectionProps {
  title?: string
  parameters: ParameterFieldConfig[]
}

export function LlmModelParametersSection({
  title,
  parameters,
}: LlmModelParametersSectionProps) {
  return (
    <>
      {title && <Title level={5}>{title}</Title>}
      {parameters.map((param, index) => (
        <LlmModelParameterField key={index} {...param} />
      ))}
    </>
  )
}
