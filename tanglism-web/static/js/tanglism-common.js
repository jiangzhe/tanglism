// 该文件定义通用的UI函数
// 依赖jquery, jquery-ui, d3

// 获取提示框，若不存在则创建
export function tooltip() {
  var t = d3.select("#k_container div.tooltip");
  if (!t.empty()) {
    return t;
  }
  return d3.select("#k_container")
    .append("div")
    .attr("class", "tooltip")
    .style("opacity", 0);
}

export function ajax_params() {
  var input_stock_code = $("#input_stock_code").val();
  var input_start_dt = $("#input_start_dt").val();
  var input_end_dt = $("#input_end_dt").val();
  var input_tick = $("#input_tick option:selected").text();
  var stroke_logic_indep_k = $("input[name='stroke_logic_indep_k']:checked").val();
  var stroke_logic_gap = $("input[name='stroke_logic_gap']:checked").val();
  var stroke_cfg;
  if (stroke_logic_indep_k === "indep_k") {
    stroke_cfg = "indep_k";
  } else {
    stroke_cfg = "indep_k:false";
  }
  if (stroke_logic_gap === "gap_opening") {
    stroke_cfg += ",gap_opening";
  } else if (stroke_logic_gap === "gap_ratio") {
    var gap_ratio = parseFloat($("#gap_ratio_percentage").val()) / 100;
    stroke_cfg += ",gap_ratio=" + gap_ratio;
  }
  return {
    tick: input_tick,
    code: input_stock_code,
    start_dt: input_start_dt,
    end_dt: input_end_dt,
    stroke_cfg
  };
}

export function validate_ajax_params() {
  var input_stock_code = $("#input_stock_code").val().trim();
  if (!input_stock_code.endsWith("XSHE") && !input_stock_code.endsWith("XSHG")) {
    return false;
  }
  if ($("#input_start_dt").val().length != 10 || $("#input_end_dt").val().length != 10) {
    return false;
  }
  return true;
}