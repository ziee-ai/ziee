import { Title } from '@ziee/kit'

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
      {/* space-y-5: inside a Card the fields aren't direct children of the Form's
          FieldGroup, so they lose its inter-field gap and each field's help text
          butts against the next field's label. Re-add the gap here. */}
      <div className="space-y-5">
        {parameters.map((param, index) => (
          <LlmModelParameterField key={index} {...param} />
        ))}
      </div>
    </>
  )
}
