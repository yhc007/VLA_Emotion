#!/usr/bin/env python3
"""
Neurosymbolic Business Reasoner
- Neural: LLM으로 엔티티/관계 추출
- Symbolic: 온톨로지 기반 추론
"""

import json
import os
from dataclasses import dataclass, field
from typing import List, Dict, Set, Optional
from pathlib import Path

# 온톨로지 로드
ONTOLOGY_PATH = Path(__file__).parent / "ontology.json"
with open(ONTOLOGY_PATH) as f:
    ONTOLOGY = json.load(f)

@dataclass
class Entity:
    text: str
    type: str
    confidence: float = 1.0

@dataclass
class Relation:
    name: str
    subject: str
    object: str
    confidence: float = 1.0

@dataclass(frozen=True)
class Fact:
    predicate: str
    args: tuple

@dataclass
class InferenceResult:
    facts: List[Fact] = field(default_factory=list)
    inferred: List[Fact] = field(default_factory=list)
    solutions: List[Dict] = field(default_factory=list)
    critical: bool = False
    priority: str = "normal"

class SymbolicReasoner:
    """온톨로지 기반 기호 추론 엔진"""
    
    def __init__(self, ontology: dict):
        self.ontology = ontology
        self.facts: Set[Fact] = set()
        self.inferred: Set[Fact] = set()
        
    def add_fact(self, predicate: str, *args):
        """사실 추가"""
        fact = Fact(predicate, args)
        self.facts.add(fact)
        
    def add_entity(self, entity: Entity):
        """엔티티를 사실로 변환"""
        # 타입 계층 처리
        self.add_fact("instanceOf", entity.text, entity.type)
        
        # 상위 클래스 추가
        for cls, info in self.ontology["classes"].items():
            if entity.type in info.get("subclasses", []):
                self.add_fact("instanceOf", entity.text, cls)
                
    def add_relation(self, relation: Relation):
        """관계를 사실로 변환"""
        self.add_fact(relation.name, relation.subject, relation.object)
        
    def query(self, predicate: str, *pattern) -> List[tuple]:
        """패턴 매칭 쿼리"""
        results = []
        all_facts = self.facts | self.inferred
        
        for fact in all_facts:
            if fact.predicate != predicate:
                continue
            if len(fact.args) != len(pattern):
                continue
                
            match = True
            bindings = {}
            for p, a in zip(pattern, fact.args):
                if p.startswith("?"):
                    bindings[p] = a
                elif p != a:
                    match = False
                    break
                    
            if match:
                results.append(fact.args)
                
        return results
    
    def infer(self) -> List[Fact]:
        """규칙 기반 추론"""
        new_inferred = []
        
        # R1: 원인-문제 연쇄 (rootCause)
        for c1 in self.query("causes", "?x", "?y"):
            for c2 in self.query("causes", c1[1], "?z"):
                fact = Fact("rootCause", (c1[0], c2[1]))
                if fact not in self.inferred:
                    self.inferred.add(fact)
                    new_inferred.append(fact)
        
        # R2: 해결책 우선순위 (highPriority)
        for sol in self.query("solves", "?s", "?p"):
            for imp in self.query("impacts", sol[1], "?i"):
                for aff in self.query("affects", imp[1], "?stakeholder"):
                    if "Customer" in aff[1] or self.query("instanceOf", aff[1], "Customer"):
                        fact = Fact("highPriority", (sol[0],))
                        if fact not in self.inferred:
                            self.inferred.add(fact)
                            new_inferred.append(fact)
        
        # R3: 리소스 제약 (blocked)
        for req in self.query("requires", "?s", "?r"):
            if self.query("unavailable", req[1]):
                fact = Fact("blocked", (req[0],))
                if fact not in self.inferred:
                    self.inferred.add(fact)
                    new_inferred.append(fact)
        
        # R4: Critical 문제 (criticalProblem)
        for p1 in self.query("impacts", "?p", "RevenueImpact"):
            for p2 in self.query("impacts", p1[0], "TimeImpact"):
                fact = Fact("criticalProblem", (p1[0],))
                if fact not in self.inferred:
                    self.inferred.add(fact)
                    new_inferred.append(fact)
        
        # R5: 영구 해결책 (permanentSolution)
        for rc in self.query("rootCause", "?x", "?p"):
            for sol in self.query("solves", "?s", rc[0]):
                fact = Fact("permanentSolution", (sol[0], rc[1]))
                if fact not in self.inferred:
                    self.inferred.add(fact)
                    new_inferred.append(fact)
        
        return new_inferred
    
    def get_result(self) -> InferenceResult:
        """추론 결과 반환"""
        self.infer()
        
        result = InferenceResult()
        result.facts = list(self.facts)
        result.inferred = list(self.inferred)
        
        # Critical 판단
        if self.query("criticalProblem", "?p"):
            result.critical = True
            result.priority = "critical"
        elif self.query("highPriority", "?s"):
            result.priority = "high"
            
        # 해결책 정리
        for sol in self.query("solves", "?s", "?p"):
            solution = {
                "name": sol[0],
                "problem": sol[1],
                "blocked": bool(self.query("blocked", sol[0])),
                "highPriority": bool(self.query("highPriority", sol[0])),
                "permanent": bool(self.query("permanentSolution", sol[0], "?p"))
            }
            result.solutions.append(solution)
            
        return result


def extract_with_llm(text: str, api_key: Optional[str] = None) -> tuple:
    """LLM으로 엔티티/관계 추출 (Claude API)"""
    
    # API 키 확인
    api_key = api_key or os.environ.get("ANTHROPIC_API_KEY")
    
    if not api_key:
        # 데모 모드: 간단한 규칙 기반 추출
        return demo_extract(text)
    
    import anthropic
    
    client = anthropic.Anthropic(api_key=api_key)
    
    prompt = f"""비즈니스 문제 분석을 위해 다음 텍스트에서 엔티티와 관계를 추출해주세요.

온톨로지 클래스:
- Problem: OperationalProblem, StrategicProblem, FinancialProblem, HRProblem, TechnicalProblem
- Cause: InternalCause, ExternalCause, ProcessCause, ResourceCause  
- Impact: RevenueImpact, CostImpact, TimeImpact, QualityImpact, ReputationImpact
- Solution: ProcessImprovement, ResourceAllocation, TechSolution, TrainingSolution, StrategicChange
- Stakeholder: Customer, Employee, Management, Partner, Investor
- Resource: HumanResource, FinancialResource, TechResource, TimeResource

관계 유형:
- causes(원인, 문제)
- impacts(문제, 영향)
- affects(영향, 이해관계자)
- solves(해결책, 문제)
- requires(해결책, 자원)

텍스트: "{text}"

JSON 형식으로 응답:
{{
  "entities": [{{"text": "...", "type": "..."}}],
  "relations": [{{"name": "...", "subject": "...", "object": "..."}}]
}}"""

    response = client.messages.create(
        model="claude-sonnet-4-20250514",
        max_tokens=1000,
        messages=[{"role": "user", "content": prompt}]
    )
    
    # JSON 파싱
    import re
    json_match = re.search(r'\{[\s\S]*\}', response.content[0].text)
    if json_match:
        result = json.loads(json_match.group())
        entities = [Entity(**e) for e in result.get("entities", [])]
        relations = [Relation(**r) for r in result.get("relations", [])]
        return entities, relations
    
    return [], []


def demo_extract(text: str) -> tuple:
    """데모용 규칙 기반 추출"""
    entities = []
    relations = []
    
    # 간단한 키워드 매칭 (확장된 버전)
    keywords = {
        # 문제 관련
        "매출": ("매출 문제", "FinancialProblem"),
        "수익": ("수익 문제", "FinancialProblem"),
        "지연": ("프로젝트 지연", "OperationalProblem"),
        "문제": ("운영 문제", "OperationalProblem"),
        "이슈": ("운영 이슈", "OperationalProblem"),
        "장애": ("시스템 장애", "TechnicalProblem"),
        "오류": ("기술 오류", "TechnicalProblem"),
        "퇴사": ("퇴사 문제", "HRProblem"),
        
        # 원인 관련
        "인력": ("인력 문제", "ResourceCause"),
        "인력수급": ("인력수급 문제", "ResourceCause"),
        "부족": ("리소스 부족", "ResourceCause"),
        "예산": ("예산 부족", "ResourceCause"),
        "일정": ("일정 압박", "ProcessCause"),
        "프로세스": ("프로세스 문제", "ProcessCause"),
        "커뮤니케이션": ("소통 문제", "ProcessCause"),
        
        # 영향 관련
        "고객": ("고객", "Customer"),
        "이탈": ("고객 이탈", "RevenueImpact"),
        "비용": ("비용 증가", "CostImpact"),
        "품질": ("품질 저하", "QualityImpact"),
        "시간": ("시간 지연", "TimeImpact"),
        "납기": ("납기 지연", "TimeImpact"),
        "신뢰": ("신뢰도 하락", "ReputationImpact"),
        
        # 해결책 관련
        "자동화": ("자동화 도입", "TechSolution"),
        "교육": ("교육 프로그램", "TrainingSolution"),
        "채용": ("인력 채용", "ResourceAllocation"),
        "외주": ("외주 활용", "ResourceAllocation"),
        "개선": ("프로세스 개선", "ProcessImprovement"),
        "해결": ("해결 방안", "ProcessImprovement"),
        "시스템": ("시스템 도입", "TechSolution"),
        "툴": ("도구 도입", "TechSolution"),
        
        # 이해관계자
        "직원": ("직원", "Employee"),
        "관리자": ("관리자", "Management"),
        "팀장": ("팀장", "Management"),
        "파트너": ("파트너", "Partner"),
        
        # 프로젝트
        "프로젝트": ("프로젝트", "OperationalProblem"),
    }
    
    found_entities = {}
    for keyword, (name, type_) in keywords.items():
        if keyword in text:
            entities.append(Entity(name, type_))
            found_entities[keyword] = name
    
    # 확장된 관계 추론
    cause_keywords = ["때문에", "으로 인해", "때문", "인해", "원인", "영향"]
    solution_keywords = ["해결", "도입", "개선", "필요", "방안", "대책"]
    
    # 원인-결과 관계
    if any(k in text for k in cause_keywords):
        causes = [e for e in entities if "Cause" in e.type]
        problems = [e for e in entities if "Problem" in e.type]
        for cause in causes:
            for problem in problems:
                relations.append(Relation("causes", cause.text, problem.text))
    
    # 해결책 관계
    if any(k in text for k in solution_keywords):
        solutions = [e for e in entities if "Solution" in e.type or "Allocation" in e.type or "Improvement" in e.type]
        problems = [e for e in entities if "Problem" in e.type]
        for sol in solutions:
            for problem in problems:
                relations.append(Relation("solves", sol.text, problem.text))
    
    # 영향 관계 (문제 → 영향)
    problems = [e for e in entities if "Problem" in e.type]
    impacts = [e for e in entities if "Impact" in e.type]
    for problem in problems:
        for impact in impacts:
            relations.append(Relation("impacts", problem.text, impact.text))
    
    # 이해관계자 영향 관계
    impacts = [e for e in entities if "Impact" in e.type]
    stakeholders = [e for e in entities if e.type in ["Customer", "Employee", "Management", "Partner"]]
    for impact in impacts:
        for stakeholder in stakeholders:
            relations.append(Relation("affects", impact.text, stakeholder.text))
    
    return entities, relations


def analyze(text: str, api_key: Optional[str] = None) -> Dict:
    """전체 분석 파이프라인"""
    
    # 1. Neural: 엔티티/관계 추출
    entities, relations = extract_with_llm(text, api_key)
    
    # 2. Symbolic: 추론 엔진
    reasoner = SymbolicReasoner(ONTOLOGY)
    
    for entity in entities:
        reasoner.add_entity(entity)
    
    for relation in relations:
        reasoner.add_relation(relation)
    
    # 3. 추론 실행
    result = reasoner.get_result()
    
    analysis = {
        "input": text,
        "entities": [{"text": e.text, "type": e.type} for e in entities],
        "relations": [{"name": r.name, "subject": r.subject, "object": r.object} for r in relations],
        "inferred": [{"predicate": f.predicate, "args": f.args} for f in result.inferred],
        "solutions": result.solutions,
        "critical": result.critical,
        "priority": result.priority,
        "summary": generate_summary(entities, relations, result)
    }

    # 그래프 데이터 첨부 (graph_visualizer 사용 가능 시)
    try:
        from graph_visualizer import SPOGraphBuilder
        builder = SPOGraphBuilder()
        builder.build_from_analysis(entities, relations, result.inferred)
        analysis["graph"] = builder.to_json()
    except ImportError:
        analysis["graph"] = None

    return analysis


def generate_summary(entities, relations, result) -> str:
    """분석 결과 요약"""
    parts = []
    
    # 문제 식별
    problems = [e for e in entities if "Problem" in e.type]
    if problems:
        parts.append(f"🔍 식별된 문제: {', '.join(e.text for e in problems)}")
    
    # 원인
    causes = [r for r in relations if r.name == "causes"]
    if causes:
        parts.append(f"📌 원인: {', '.join(f'{r.subject}→{r.object}' for r in causes)}")
    
    # 영향
    if result.critical:
        parts.append("⚠️ 상태: CRITICAL (수익+시간 영향)")
    elif result.priority == "high":
        parts.append("🔶 상태: 고객 영향 - 우선순위 높음")
    
    # 해결책
    if result.solutions:
        for sol in result.solutions:
            status = "✅" if not sol["blocked"] else "🚫 차단됨"
            if sol["permanent"]:
                status += " (영구 해결책)"
            parts.append(f"💡 해결책: {sol['name']} {status}")
    
    return "\n".join(parts) if parts else "분석 결과 없음"


# CLI
if __name__ == "__main__":
    import sys

    args = sys.argv[1:]
    generate_graph = False

    if "--graph" in args:
        generate_graph = True
        args.remove("--graph")

    if args:
        text = " ".join(args)
    else:
        text = "인력 부족 때문에 프로젝트가 지연되고 있어서 자동화 도입을 검토 중입니다"

    print(f"📝 입력: {text}\n")
    result = analyze(text)

    print("=" * 50)
    print(result["summary"])
    print("=" * 50)
    print(f"\n📊 상세 결과:")
    print(json.dumps(result, indent=2, ensure_ascii=False, default=str))

    # --graph 옵션: HTML 시각화 생성
    if generate_graph:
        try:
            from graph_visualizer import analyze_and_visualize
            viz = analyze_and_visualize(text)
            print(f"\n📈 SPO 그래프 생성 완료:")
            print(f"   HTML: {viz['html_path']}")
            print(f"   JSON: {viz['json_path']}")
        except ImportError as e:
            print(f"\n⚠️ 그래프 시각화 실패: {e}")
