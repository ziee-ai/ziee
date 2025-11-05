import { Typography } from 'antd'
import {
  LlmModelParameterField,
  ParameterFieldConfig,
} from '@/components/common/LlmModelParameterField'

const { Title } = Typography

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
